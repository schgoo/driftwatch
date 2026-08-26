//! Shared helpers for the annotation macros: the `__rt` funnel path, return-type
//! classification, parameter-rename (`#[watch_input]`) extraction, type
//! predicates, and identifier sanitizing.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, LitStr, Pat, ReturnType, Type};

/// The funnel path the generated code references. Everything the macros expand
/// into is reached through the facade's hidden `__rt` module, so annotated
/// crates never name `runtime` or `linkme`.
pub fn rt() -> TokenStream2 {
    quote! { ::annotations::__rt }
}

/// The token for an item's `component` field: the annotated crate's package
/// name, taken from `CARGO_PKG_NAME` at the annotated crate's compile time.
/// Extraction groups operations and types by this component.
pub fn component_tokens() -> TokenStream2 {
    quote! { ::core::env!("CARGO_PKG_NAME") }
}

/// How an operation's return type is emitted as `$result`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReturnKind {
    Unit,
    Result,
    Option,
    Other,
}

/// Classify a return type by the outermost path segment (`Result`/`Option`),
/// treating `()`/absent as `Unit` and everything else as `Other`.
pub fn classify_return(ty: &ReturnType) -> ReturnKind {
    match ty {
        ReturnType::Default => ReturnKind::Unit,
        ReturnType::Type(_, t) => match &**t {
            Type::Tuple(t) if t.elems.is_empty() => ReturnKind::Unit,
            Type::Path(p) => match p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .as_deref()
            {
                Some("Result") => ReturnKind::Result,
                Some("Option") => ReturnKind::Option,
                _ => ReturnKind::Other,
            },
            _ => ReturnKind::Other,
        },
    }
}

/// True when the function has a `self` receiver (i.e. it is a method).
pub fn has_receiver(f: &ItemFn) -> bool {
    f.sig.inputs.iter().any(|a| matches!(a, FnArg::Receiver(_)))
}

fn is_owned_primitive(ty: &Type) -> bool {
    if let Type::Path(p) = ty
        && let Some(s) = p.path.segments.last()
    {
        return matches!(
            s.ident.to_string().as_str(),
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
                | "bool"
                | "char"
                | "String"
                | "str"
        );
    }
    false
}

/// True for `&mut T` parameters. These represent mutable state objects threaded
/// through an operation (their mutations are captured separately), not value
/// inputs — so they are excluded from input-echo emission.
pub fn is_mut_ref(ty: &Type) -> bool {
    matches!(ty, Type::Reference(r) if r.mutability.is_some())
}

/// True for owned primitives and shared references to primitives (notably
/// `&str`) — the printed value goes through `format!("{}", x)`.
pub fn is_printable_param(ty: &Type) -> bool {
    if is_owned_primitive(ty) {
        return true;
    }
    if let Type::Reference(r) = ty {
        return is_owned_primitive(&r.elem);
    }
    false
}

/// A typed parameter with its code identifier, declared type, and optional
/// `#[watch_input("name")]` override.
pub struct Param {
    pub ident: Ident,
    pub ty: Type,
    pub rename: Option<String>,
}

/// Extract each typed parameter, consuming and removing any
/// `#[watch_input("name")]` attribute. The override (when present) is the
/// event name the parameter emits under; the code identifier is otherwise used.
pub fn extract_param_renames(f: &mut ItemFn) -> Vec<Param> {
    let mut out = Vec::new();
    for arg in &mut f.sig.inputs {
        if let FnArg::Typed(pt) = arg
            && let Pat::Ident(id) = &*pt.pat
        {
            let ident = id.ident.clone();
            let ty = (*pt.ty).clone();
            let mut rename = None;
            pt.attrs.retain(|a| {
                if a.path().is_ident("watch_input") {
                    if let Ok(s) = a.parse_args::<LitStr>() {
                        rename = Some(s.value());
                    }
                    false
                } else {
                    true
                }
            });
            out.push(Param { ident, ty, rename });
        }
    }
    out
}

/// The emitted name of a parameter: its `#[watch_input]` override, else the
/// code identifier.
pub fn param_name(p: &Param) -> String {
    p.rename.clone().unwrap_or_else(|| p.ident.to_string())
}

/// Turn an arbitrary string into a valid uppercase identifier suffix.
pub fn sanitize_ident(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}
