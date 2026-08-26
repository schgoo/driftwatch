//! The `#[watch_operation]` attribute expansion.
//!
//! The operation name is the function name; the component comes from
//! `watch_component!`. Expansion emits a `Run` marker, one event per value
//! parameter, instruments the body (field-mutation echo), emits `$result` after the body on every return path, and registers
//! an [`OpMeta`](crate::shared) entry into the link-time registry. Unlike the
//! `SpecGate` `#[spec_operation]` it lifts from, it takes no arguments (the
//! `spec = "…"` component override is dropped).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Block, Ident, ItemFn, ReturnType, Stmt, Type, parse_macro_input, parse_quote};

use crate::body::BodyInstrumenter;
use crate::shared::{
    Param, ReturnKind, classify_return, component_tokens, extract_param_renames, has_receiver,
    is_mut_ref, is_printable_param, param_name, rt,
};

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(item as ItemFn);

    let op_name = func.sig.ident.to_string();
    let is_async = func.sig.asyncness.is_some();
    let _is_method = has_receiver(&func);
    let params = extract_param_renames(&mut func);
    let param_names: Vec<String> = params.iter().map(|p| p.ident.to_string()).collect();

    let mut visitor = BodyInstrumenter {
        param_names: param_names.clone(),
    };
    visitor.visit_block_mut(&mut func.block);
    let body = &func.block;

    let pre = build_pre_stmts(&op_name, &params);
    // Post-body emission of `$result` (and, for struct returns, per-field
    // events). Wrapping the body ensures the emission runs on EVERY return
    // path, including early `return`s and `?` short-circuits.
    let post = build_post_emit(&func.sig.output);
    let new_body: Block = if let Some(post) = post {
        let ret_ty = match &func.sig.output {
            ReturnType::Type(_, ty) => ty.clone(),
            ReturnType::Default => unreachable!("post-emit only built for a non-unit return"),
        };
        if is_async {
            parse_quote!({
                #(#pre)*
                #[allow(clippy::redundant_closure_call, reason = "uniform body wrapper for return-path emission")]
                let __dw_ret = (async move #body).await;
                #post
                __dw_ret
            })
        } else {
            parse_quote!({
                #(#pre)*
                #[allow(clippy::redundant_closure_call, reason = "uniform body wrapper for return-path emission")]
                let __dw_ret = (move || -> #ret_ty #body)();
                #post
                __dw_ret
            })
        }
    } else {
        parse_quote!({
            #(#pre)*
            #body
        })
    };
    *func.block = new_body;

    let registration = build_registration(&func, &op_name, is_async, &params);

    quote! {
        #func
        #registration
    }
    .into()
}

/// Pre-body statements: the `Run` marker plus one `op.<name>` event per value
/// parameter. Receivers and `&mut T` parameters are excluded (receivers are not
/// in `params`; mutable refs are state, not inputs).
fn build_pre_stmts(op_name: &str, params: &[Param]) -> Vec<Stmt> {
    let rt = rt();
    let mut out: Vec<Stmt> = vec![parse_quote!(#rt::emit_run(#op_name);)];
    for p in params {
        if is_mut_ref(&p.ty) {
            continue;
        }
        let name = param_name(p);
        let event_name = format!("{op_name}.{name}");
        let id = &p.ident;
        out.push(parse_quote!(
            #rt::emit_event_v(#event_name, #rt::ToValue::to_value(&#id));
        ));
    }
    out
}

/// Build the post-body `$result`/field emission for an operation.
///
/// Returns `None` for a unit/`()` return. Otherwise emits, from the captured
/// `__dw_ret`:
/// - `Result<T, E>` → a tagged `{Ok|Err}` map `$result`.
/// - `Option<T>`    → a tagged `{Some|None}` map `$result`.
/// - a printable scalar → a `Display`-string `$result`.
/// - anything else  → the `ReturnEmit` autoref ladder (per-field events + a
///   structured `$result` for struct returns; a structured `$result` for
///   enums/collections; a `Display` `$result` as a last resort).
fn build_post_emit(output: &ReturnType) -> Option<TokenStream2> {
    let rt = rt();
    let ty = match output {
        ReturnType::Default => return None,
        ReturnType::Type(_, t) => {
            if matches!(&**t, Type::Tuple(tup) if tup.elems.is_empty()) {
                return None;
            }
            t
        }
    };
    Some(match classify_return(output) {
        ReturnKind::Unit => return None,
        ReturnKind::Result => quote! {
            match &__dw_ret {
                ::core::result::Result::Ok(__dw_v) => {
                    let mut __dw_m = ::std::collections::BTreeMap::new();
                    __dw_m.insert("Ok".to_string(), #rt::ToValue::to_value(__dw_v));
                    #rt::emit_event_v("$result", #rt::Value::Map(__dw_m));
                }
                ::core::result::Result::Err(__dw_e) => {
                    let mut __dw_m = ::std::collections::BTreeMap::new();
                    __dw_m.insert("Err".to_string(), #rt::Value::String(::std::format!("{}", __dw_e)));
                    #rt::emit_event_v("$result", #rt::Value::Map(__dw_m));
                }
            }
        },
        ReturnKind::Option => quote! {
            match &__dw_ret {
                ::core::option::Option::Some(__dw_v) => {
                    let mut __dw_m = ::std::collections::BTreeMap::new();
                    __dw_m.insert("Some".to_string(), #rt::ToValue::to_value(__dw_v));
                    #rt::emit_event_v("$result", #rt::Value::Map(__dw_m));
                }
                ::core::option::Option::None => {
                    let mut __dw_m = ::std::collections::BTreeMap::new();
                    __dw_m.insert("None".to_string(), #rt::Value::Map(::std::collections::BTreeMap::new()));
                    #rt::emit_event_v("$result", #rt::Value::Map(__dw_m));
                }
            }
        },
        ReturnKind::Other => {
            if is_printable_param(ty) {
                quote! {
                    #rt::emit_event_v("$result", #rt::ToValue::to_value(&__dw_ret));
                }
            } else {
                quote! {
                    {
                        use #rt::ReturnEmitStruct as _;
                        use #rt::ReturnEmitToValue as _;
                        use #rt::ReturnEmitDisplay as _;
                        use #rt::ReturnEmitNone as _;
                        (&&&&#rt::ReturnEmit(&__dw_ret)).emit_result();
                    }
                }
            }
        }
    })
}

/// The link-time `OpMeta` registration, wrapped in a named `const` so it
/// compiles both at module scope and inside an `impl` block.
fn build_registration(
    func: &ItemFn,
    op_name: &str,
    is_async: bool,
    params: &[Param],
) -> TokenStream2 {
    let rt = rt();
    let component = component_tokens();
    let fn_name = func.sig.ident.to_string();
    let const_ident = Ident::new(
        &format!("_DRIFTWATCH_REG_{}", fn_name.to_uppercase()),
        func.sig.ident.span(),
    );
    let static_ident = Ident::new(
        &format!("_DRIFTWATCH_STATIC_{}", fn_name.to_uppercase()),
        func.sig.ident.span(),
    );
    let param_entries: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name_str = param_name(p);
            let ty = &p.ty;
            let ty_str = quote!(#ty).to_string();
            quote! { (#name_str, #ty_str) }
        })
        .collect();
    let ret_str = match &func.sig.output {
        ReturnType::Default => String::from("()"),
        ReturnType::Type(_, ty) => quote!(#ty).to_string(),
    };

    quote! {
        #[allow(dead_code, non_upper_case_globals, reason = "macro-generated registry scaffolding")]
        const #const_ident: () = {
            #[#rt::linkme::distributed_slice(#rt::DRIFTWATCH_OPS)]
            #[linkme(crate = #rt::linkme)]
            static #static_ident: #rt::OpMeta = #rt::OpMeta {
                name: #op_name,
                module_path: ::core::module_path!(),
                fn_name: #fn_name,
                is_setup: false,
                is_async: #is_async,
                params: &[#(#param_entries),*],
                return_type: #ret_str,
                fills: "",
                component: #component,
            };
        };
    }
}
