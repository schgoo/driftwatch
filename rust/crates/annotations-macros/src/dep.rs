//! The `watch_dep!("name", <expr>)` function-like macro.
//!
//! `watch_dep!` is a transparent observer in expression position: it runs the
//! wrapped call, emits a nested `conformance.operation` span (own inputs +
//! completion) as a side effect, and returns the wrapped expression's value
//! **unchanged** (`Result`→`Result`, `Option`→`Option`, `T`→`T`). Combinators
//! and `?` compose *outside* the macro; a trailing `.await` lives *inside*.
//!
//! For `watch_dep!("parse", i64::from_str_radix(text, 16))` the expansion:
//!
//! - binds each call argument once to a temp (no double-eval), borrows it for
//!   `ToValue` capture keyed by identifier (`text`) or positionally (`arg1`),
//!   then moves it into the reconstructed call;
//! - opens a child span via `open_dep` whose component is the optional
//!   `component = "…"` override, else the enclosing operation's (a runtime stack
//!   read);
//! - runs the real call inside `catch_unwind` (sync) / `catch_unwind_fut`
//!   (async), so a panic is dispositioned as a `conformance.fault` on the dep
//!   span and re-propagated (the ratified Option A cascade);
//! - dispositions the returned value through the runtime `DepObserve` ladder
//!   (`Ok`/`Some`→`result`, `Err`→`error`, `None`→`empty`) *before* handing it
//!   back, so an `Err` is recorded even though the caller's `?` unwraps outside;
//! - closes the child span and evaluates to the original value.
//!
//! A trailing `.await` is peeled for input-capture/observation and re-applied
//! inside the awaited path; `?` and other combinators are the author's to place
//! outside. A non-call initializer (`watch_dep!("cfg", CONFIG)`) captures zero
//! inputs and is value-only. A wrapped argument that is not `ToValue` is a
//! natural compile error — the intended boundary guard.
//!
//! With `trace` off the macro is a true identity: it expands to the wrapped
//! expression verbatim, with no temps, no bindings, and no `__rt` references.

use proc_macro::TokenStream;
#[cfg(feature = "trace")]
use proc_macro2::TokenStream as TokenStream2;
#[cfg(feature = "trace")]
use quote::format_ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, Token, parse_macro_input};

/// The parsed `watch_dep!("name"[, component = "…"], <expr>)` invocation. The
/// name is mandatory; the component override is optional (a keyword arg); the
/// expression is the wrapped call.
#[cfg_attr(
    not(feature = "trace"),
    allow(
        dead_code,
        reason = "name/component are read only by the trace-on expansion; the identity path uses only `expr`"
    )
)]
pub struct DepCall {
    /// The dependency (nested-operation) name.
    name: LitStr,
    /// An explicit component override, or `None` to inherit the parent's.
    component: Option<LitStr>,
    /// The wrapped expression (the real dependency call).
    expr: Expr,
}

impl Parse for DepCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let mut component = None;
        // `component = "…"` is disambiguated from an expression by the `ident =`
        // prefix; a call or path never starts `ident =`.
        if input.peek(syn::Ident) && input.peek2(Token![=]) {
            let key: syn::Ident = input.parse()?;
            if key != "component" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `component = \"...\"`",
                ));
            }
            input.parse::<Token![=]>()?;
            component = Some(input.parse::<LitStr>()?);
            input.parse::<Token![,]>()?;
        }
        let expr: Expr = input.parse()?;
        Ok(DepCall {
            name,
            component,
            expr,
        })
    }
}

/// Trace-off identity: expand to the wrapped expression verbatim.
#[cfg(not(feature = "trace"))]
pub fn expand_identity(input: TokenStream) -> TokenStream {
    let DepCall { expr, .. } = parse_macro_input!(input as DepCall);
    quote!(#expr).into()
}

/// Trace-on expansion: the transparent-observer block.
#[cfg(feature = "trace")]
pub fn expand(input: TokenStream) -> TokenStream {
    use crate::shared::{fault_arm, rt};

    let DepCall {
        name,
        component,
        expr,
    } = parse_macro_input!(input as DepCall);
    let rt = rt();

    let component_tok = if let Some(c) = &component {
        quote! { ::core::option::Option::Some(#c) }
    } else {
        quote! { ::core::option::Option::None }
    };

    // Peel only a trailing `.await`; re-applied inside the awaited path.
    let (peeled, is_await): (&Expr, bool) = if let Expr::Await(a) = &expr {
        (a.base.as_ref(), true)
    } else {
        (&expr, false)
    };

    let Parts {
        temp_lets,
        input_inserts,
        call_expr,
    } = build_call(peeled);

    let fault = fault_arm();
    let run = if is_await {
        quote! {
            match #rt::catch_unwind_fut(async move { #call_expr.await }).await {
                ::core::result::Result::Ok(__dw_v) => __dw_v,
                ::core::result::Result::Err(__dw_payload) => #fault,
            }
        }
    } else {
        quote! {
            match ::std::panic::catch_unwind(
                ::std::panic::AssertUnwindSafe(move || #call_expr),
            ) {
                ::core::result::Result::Ok(__dw_v) => __dw_v,
                ::core::result::Result::Err(__dw_payload) => #fault,
            }
        }
    };

    quote! {
        {
            #(#temp_lets)*
            let mut __dw_inputs = ::std::collections::BTreeMap::new();
            #(#input_inserts)*
            let __dw_dep = #rt::open_dep(#name, #component_tok, __dw_inputs);
            let __dw_res = #run;
            {
                use #rt::DepObserveResult as _;
                use #rt::DepObserveOption as _;
                use #rt::DepObserveResultDebug as _;
                use #rt::DepObserveOptionDebug as _;
                use #rt::DepObserveToValue as _;
                use #rt::DepObserveDisplay as _;
                use #rt::DepObserveDebug as _;
                use #rt::DepObserveOther as _;
                (&&&&&&&#rt::DepObserve(&__dw_res)).observe();
            }
            ::core::mem::drop(__dw_dep);
            __dw_res
        }
    }
    .into()
}

/// The pieces of a reconstructed dependency call: temp bindings that evaluate
/// each argument (and a method receiver) once, the `conformance.operation.inputs`
/// inserts that borrow those temps, and the call expression that moves them.
#[cfg(feature = "trace")]
struct Parts {
    temp_lets: Vec<TokenStream2>,
    input_inserts: Vec<TokenStream2>,
    call_expr: TokenStream2,
}

/// Split a call into per-argument temps + input captures + a reconstructed call.
///
/// A method call binds its receiver first (evaluation order) then each arg; a
/// free call binds each arg and keeps the callee verbatim. Every argument is
/// captured as an input (identifier-keyed, else positional); the receiver is
/// bound but never captured. A non-call expression captures nothing and is used
/// verbatim.
#[cfg(feature = "trace")]
fn build_call(peeled: &Expr) -> Parts {
    use crate::shared::rt;
    let rt = rt();
    match peeled {
        Expr::MethodCall(mc) => {
            let recv = &mc.receiver;
            let method = &mc.method;
            let turbofish = &mc.turbofish;
            let mut temp_lets = vec![quote! { let __dw_recv = #recv; }];
            let mut input_inserts = Vec::new();
            let mut arg_idents = Vec::new();
            for (i, a) in mc.args.iter().enumerate() {
                let id = format_ident!("__dw_arg{}", i);
                temp_lets.push(quote! { let #id = #a; });
                let name = arg_name(i, a);
                input_inserts.push(quote! {
                    __dw_inputs.insert(#name.to_string(), #rt::ToValue::to_value(&#id));
                });
                arg_idents.push(id);
            }
            let call_expr = quote! { __dw_recv.#method #turbofish (#(#arg_idents),*) };
            Parts {
                temp_lets,
                input_inserts,
                call_expr,
            }
        }
        Expr::Call(c) => {
            let func = &c.func;
            let mut temp_lets = Vec::new();
            let mut input_inserts = Vec::new();
            let mut arg_idents = Vec::new();
            for (i, a) in c.args.iter().enumerate() {
                let id = format_ident!("__dw_arg{}", i);
                temp_lets.push(quote! { let #id = #a; });
                let name = arg_name(i, a);
                input_inserts.push(quote! {
                    __dw_inputs.insert(#name.to_string(), #rt::ToValue::to_value(&#id));
                });
                arg_idents.push(id);
            }
            let call_expr = quote! { (#func)(#(#arg_idents),*) };
            Parts {
                temp_lets,
                input_inserts,
                call_expr,
            }
        }
        other => Parts {
            temp_lets: Vec::new(),
            input_inserts: Vec::new(),
            call_expr: quote! { #other },
        },
    }
}

/// The input-name for the `i`th argument: its identifier when it is a bare path
/// (peeling a leading `&`/`&mut`), otherwise the positional `arg{i}`.
#[cfg(feature = "trace")]
fn arg_name(i: usize, a: &Expr) -> String {
    match a {
        Expr::Reference(r) => arg_name(i, &r.expr),
        Expr::Path(p) => match p.path.get_ident() {
            Some(id) => id.to_string(),
            None => format!("arg{i}"),
        },
        _ => format!("arg{i}"),
    }
}
