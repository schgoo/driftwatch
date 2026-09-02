//! The `#[watch_dep("name", component = "...")]` `let`-binding rewrite.
//!
//! Applied to a `let` inside a `#[watch_operation]` body; purely observational.
//! For `let x = dep.call(a, &b)?;` it expands to a **nested
//! `conformance.operation` span**:
//!
//! - open a child span named `name` whose `conformance.operation.inputs` kvlist
//!   carries one entry per argument (by identifier, `&`/`&mut` peeled; else
//!   positional `arg{i}`); its `conformance.component.id` is the dep's declared
//!   component else the enclosing operation's;
//! - the original call, run verbatim and bound to a temp;
//! - the child span's completion event — `result` (Ok, unwrapped) or `error`
//!   (Err, decomposed like an operation's, with the fallback name `"error"`);
//! - close the child span, then re-apply the original pattern and any trailing
//!   `?`.
//!
//! The real call always runs and its `Result` binds unchanged: no table, no
//! substitution. The `?` is observed before it unwraps, so an `Err` is recorded
//! then propagated. Arguments are re-evaluated in the call, so they must be
//! side-effect-free (identifiers, literals, field accesses).
//!
//! Only a direct function or method call (optionally one trailing `?`) is
//! supported; `.await`, chained combinators, and non-call initializers produce a
//! `compile_error!`.

use syn::{Block, Expr, Local, Stmt, parse_quote};

use crate::shared::{DepArgs, rt};

/// Diagnostic emitted when `#[watch_dep]` is attached to an unsupported
/// initializer shape.
const UNSUPPORTED_SHAPE: &str = "#[watch_dep] expects a direct function or method call as the initializer (an optional trailing `?` is allowed); `.await`, chained calls, and other initializer shapes are not supported";

/// If `attrs` carry a `#[watch_dep("name", ...)]`, return the parsed args.
pub fn take_dep_args(attrs: &[syn::Attribute]) -> Option<DepArgs> {
    for a in attrs {
        if a.path().is_ident("watch_dep")
            && let Ok(args) = a.parse_args::<DepArgs>()
        {
            return Some(args);
        }
    }
    None
}

/// Expand a `#[watch_dep]` `let` into a nested `conformance.operation` span
/// around the real call (trailing `?` preserved). `None` when there is no
/// initializer, or it is not a call after peeling `?`. `parent_component` is the
/// enclosing operation's effective component, inherited when the dep declares no
/// override.
pub fn expand_dep_let(local: &Local, args: &DepArgs, parent_component: &str) -> Option<Vec<Stmt>> {
    let init = local.init.as_ref()?;

    // Peel a trailing `?`; re-applied verbatim after observation.
    let (inner, is_try): (&Expr, bool) = match &*init.expr {
        Expr::Try(t) => (&t.expr, true),
        other => (other, false),
    };

    let inputs = dep_inputs(inner)?;
    let rt = rt();
    let pat = &local.pat;
    let dep_name = &args.name;
    let dep_component: &str = args.component.as_deref().unwrap_or(parent_component);

    let input_inserts: Vec<proc_macro2::TokenStream> = inputs
        .iter()
        .map(|(name, expr)| {
            quote::quote! {
                __dw_inputs.insert(#name.to_string(), #rt::ToValue::to_value(&(#expr)));
            }
        })
        .collect();

    // Observe the `Result` by borrow inside the child span, then close it and
    // apply the original binding and `?`.
    let maybe_q: Option<syn::Token![?]> = is_try.then(|| parse_quote!(?));
    let call: Block = parse_quote!({
        let mut __dw_inputs = ::std::collections::BTreeMap::new();
        #(#input_inserts)*
        let __dw_dep = #rt::open_operation(#dep_name, #dep_component, __dw_inputs);
        let __dw_res = (#inner);
        match &__dw_res {
            ::core::result::Result::Ok(__dw_v) => {
                #rt::push_result(#rt::ToValue::to_value(__dw_v));
            }
            ::core::result::Result::Err(__dw_e) => {
                use #rt::ValueEmitToValue as _;
                use #rt::ValueEmitDisplay as _;
                use #rt::ValueEmitDebug as _;
                use #rt::ValueEmitUniversal as _;
                let __dw_val = (&&&&#rt::ValueEmit(&__dw_e)).encode();
                let (__dw_n, __dw_ev) = #rt::split_error(__dw_val, "error");
                #rt::push_error(__dw_n, __dw_ev);
            }
        };
        ::core::mem::drop(__dw_dep);
        let #pat = __dw_res #maybe_q;
    });
    Some(call.stmts)
}

/// Each call argument paired with its event-name suffix. `None` when the
/// initializer is not a direct function or method call.
fn dep_inputs(e: &Expr) -> Option<Vec<(String, &Expr)>> {
    let args = match e {
        Expr::MethodCall(mc) => &mc.args,
        Expr::Call(c) => &c.args,
        _ => return None,
    };
    Some(
        args.iter()
            .enumerate()
            .map(|(i, a)| (arg_name(i, a), a))
            .collect(),
    )
}

/// The event-name suffix for the `i`th argument: its identifier when it is a
/// bare path (peeling a leading `&`/`&mut`), otherwise the positional `arg{i}`.
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

/// Rewrite a `#[watch_dep]`-tagged `let`: expanded statements on success;
/// unsupported shapes strip the attribute and prepend a `compile_error!`.
/// Untagged `let`s pass through unchanged. `parent_component` is the enclosing
/// operation's effective component.
pub fn rewrite_local(local: Local, parent_component: &str) -> Vec<Stmt> {
    let Some(args) = take_dep_args(&local.attrs) else {
        return vec![Stmt::Local(local)];
    };
    if let Some(stmts) = expand_dep_let(&local, &args, parent_component) {
        return stmts;
    }
    let mut stripped = local;
    stripped.attrs.retain(|a| !a.path().is_ident("watch_dep"));
    let msg = syn::LitStr::new(UNSUPPORTED_SHAPE, proc_macro2::Span::call_site());
    let err: Stmt = parse_quote! { ::core::compile_error!(#msg); };
    vec![err, Stmt::Local(stripped)]
}
