//! The `#[watch_operation(component = "...")]` attribute expansion.
//!
//! The operation name is the function name; the component is author-declared
//! (mandatory) and supplies both the span's `conformance.component.id` and the
//! registry `OpMeta.component`. Expansion opens a `conformance.operation` span
//! (inputs as one kvlist attribute keyed by bare identifier), instruments the
//! body (field-mutation → `conformance.observation`), pushes the completion
//! event (`result`/`empty`/`error`) after the body on every return path, and
//! registers an [`OpMeta`](crate::shared) entry into the link-time registry.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Block, Ident, ItemFn, ReturnType, Stmt, Type, parse_macro_input, parse_quote};

use crate::body::BodyInstrumenter;
use crate::shared::{
    OperationArgs, Param, ReturnKind, classify_return, extract_param_renames, has_receiver,
    is_mut_ref, is_printable_param, param_name, result_error_name, rt,
};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as OperationArgs);
    let component = args.component;
    let mut func = parse_macro_input!(item as ItemFn);

    let op_name = func.sig.ident.to_string();
    let is_async = func.sig.asyncness.is_some();
    let _is_method = has_receiver(&func);
    let params = extract_param_renames(&mut func);
    let param_names: Vec<String> = params.iter().map(|p| p.ident.to_string()).collect();

    let mut visitor = BodyInstrumenter {
        param_names: param_names.clone(),
        component: component.clone(),
    };
    visitor.visit_block_mut(&mut func.block);
    let body = &func.block;

    let open = build_open(&op_name, &component, &params);
    // Post-body completion event. Wrapping the body ensures the emission runs on
    // EVERY return path, including early `return`s and `?` short-circuits.
    let post = build_post_emit(&func.sig.output);
    let new_body: Block = if let Some(post) = post {
        let ret_ty = match &func.sig.output {
            ReturnType::Type(_, ty) => ty.clone(),
            ReturnType::Default => unreachable!("post-emit only built for a non-unit return"),
        };
        if is_async {
            parse_quote!({
                #(#open)*
                #[allow(clippy::redundant_closure_call, reason = "uniform body wrapper for return-path emission")]
                let __dw_ret = (async move #body).await;
                #post
                __dw_ret
            })
        } else {
            parse_quote!({
                #(#open)*
                #[allow(clippy::redundant_closure_call, reason = "uniform body wrapper for return-path emission")]
                let __dw_ret = (move || -> #ret_ty #body)();
                #post
                __dw_ret
            })
        }
    } else {
        parse_quote!({
            #(#open)*
            #body
        })
    };
    *func.block = new_body;

    let registration = build_registration(&func, &op_name, is_async, &component, &params);

    quote! {
        #func
        #registration
    }
    .into()
}

/// Open the operation span: build the `conformance.operation.inputs` kvlist
/// (bare-identifier keys; receivers and `&mut T` parameters excluded) and bind a
/// span guard that lives to the end of the wrapped body.
fn build_open(op_name: &str, component: &str, params: &[Param]) -> Vec<Stmt> {
    let rt = rt();
    let mut inserts: Vec<TokenStream2> = Vec::new();
    for p in params {
        if is_mut_ref(&p.ty) {
            continue;
        }
        let name = param_name(p);
        let id = &p.ident;
        inserts.push(quote! {
            __dw_inputs.insert(#name.to_string(), #rt::ToValue::to_value(&#id));
        });
    }
    let block: Block = parse_quote!({
        let mut __dw_inputs = ::std::collections::BTreeMap::new();
        #(#inserts)*
        let _dw_op = #rt::open_operation(#op_name, #component, __dw_inputs);
    });
    block.stmts
}

/// Build the post-body completion event for an operation.
///
/// Returns `None` for a unit/`()` return (no completion event). Otherwise, from
/// the captured `__dw_ret`:
/// - `Result<T, E>` → `result` (Ok, unwrapped) / `error` (Err, decomposed).
/// - `Result<Option<T>, E>` → `result` (Ok(Some)) / `empty` (Ok(None)) /
///   `error` (Err).
/// - `Option<T>` → `result` (Some) / `empty` (None).
/// - a printable scalar → a direct `result`.
/// - anything else → the `ValueEmit` autoref ladder (one structured `result`).
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
    let fallback = result_error_name(output);
    let err_arm = quote! {
        ::core::result::Result::Err(__dw_e) => {
            use #rt::ValueEmitToValue as _;
            use #rt::ValueEmitDisplay as _;
            use #rt::ValueEmitDebug as _;
            use #rt::ValueEmitUniversal as _;
            let __dw_val = (&&&&#rt::ValueEmit(&__dw_e)).encode();
            let (__dw_name, __dw_ev) = #rt::split_error(__dw_val, #fallback);
            #rt::push_error(__dw_name, __dw_ev);
        }
    };
    Some(match classify_return(output) {
        ReturnKind::Unit => return None,
        ReturnKind::Result => quote! {
            match &__dw_ret {
                ::core::result::Result::Ok(__dw_v) => #rt::push_result(#rt::ToValue::to_value(__dw_v)),
                #err_arm
            }
        },
        ReturnKind::ResultOption => quote! {
            match &__dw_ret {
                ::core::result::Result::Ok(::core::option::Option::Some(__dw_v)) => {
                    #rt::push_result(#rt::ToValue::to_value(__dw_v));
                }
                ::core::result::Result::Ok(::core::option::Option::None) => #rt::push_empty(),
                #err_arm
            }
        },
        ReturnKind::Option => quote! {
            match &__dw_ret {
                ::core::option::Option::Some(__dw_v) => {
                    #rt::push_result(#rt::ToValue::to_value(__dw_v));
                }
                ::core::option::Option::None => #rt::push_empty(),
            }
        },
        ReturnKind::Other => {
            if is_printable_param(ty) {
                quote! {
                    #rt::push_result(#rt::ToValue::to_value(&__dw_ret));
                }
            } else {
                quote! {
                    {
                        use #rt::ValueEmitToValue as _;
                        use #rt::ValueEmitDisplay as _;
                        use #rt::ValueEmitDebug as _;
                        use #rt::ValueEmitUniversal as _;
                        #rt::push_result((&&&&#rt::ValueEmit(&__dw_ret)).encode());
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
    component: &str,
    params: &[Param],
) -> TokenStream2 {
    let rt = rt();
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
