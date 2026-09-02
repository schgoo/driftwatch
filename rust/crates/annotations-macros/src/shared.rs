//! Shared helpers for the annotation macros: the `__rt` funnel path, return-type
//! classification, parameter-rename (`#[watch_input]`) extraction, type
//! predicates, and identifier sanitizing.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::ParseStream;
use syn::{
    FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, PathArguments, ReturnType, Token, Type,
};

/// The funnel path the generated code references. Everything the macros expand
/// into is reached through the facade's hidden `__rt` module, so annotated
/// crates never name `runtime` or `linkme`.
pub fn rt() -> TokenStream2 {
    quote! { ::annotations::__rt }
}

/// Diagnostic when `#[watch_operation]` is applied without a component.
pub const MISSING_COMPONENT: &str =
    "`#[watch_operation]` requires a component: `#[watch_operation(component = \"...\")]`";

/// The arguments of `#[watch_operation(component = "...")]`. The component is
/// **mandatory**: it supplies both the span's `conformance.component.id` and the
/// registry `OpMeta.component`, and is language-agnostic (a Rust and a C#
/// implementation of the same component declare the same id).
#[derive(Debug)]
pub struct OperationArgs {
    /// The author-declared component id.
    pub component: String,
}

impl syn::parse::Parse for OperationArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                MISSING_COMPONENT,
            ));
        }
        let key: Ident = input.parse()?;
        if key != "component" {
            return Err(syn::Error::new(key.span(), MISSING_COMPONENT));
        }
        let _: Token![=] = input.parse()?;
        let value: LitStr = input.parse()?;
        let component = value.value();
        if component.is_empty() {
            return Err(syn::Error::new(value.span(), MISSING_COMPONENT));
        }
        Ok(Self { component })
    }
}

/// The arguments of `#[watch_dep("name", component = "...")]`. The name is
/// mandatory; the component is optional and defaults to the enclosing
/// operation's effective component.
#[derive(Debug)]
pub struct DepArgs {
    /// The dependency (nested-operation) name.
    pub name: String,
    /// An explicit component override, or `None` to inherit the parent's.
    pub component: Option<String>,
}

impl syn::parse::Parse for DepArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<LitStr>()?.value();
        let mut component = None;
        if input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
            let key: Ident = input.parse()?;
            if key != "component" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `component = \"...\"`",
                ));
            }
            let _: Token![=] = input.parse()?;
            component = Some(input.parse::<LitStr>()?.value());
        }
        Ok(Self { name, component })
    }
}

/// The CTSC `error.name` fallback for a `Result<T, E>` return: the last path
/// segment of `E` (stringified). Used when the decomposed error value is not a
/// tagged variant. Falls back to `"error"` when `E` is not an inspectable path.
pub fn result_error_name(output: &ReturnType) -> String {
    if let ReturnType::Type(_, ty) = output
        && let Some(err) = result_err_type(ty)
        && let Type::Path(p) = err
        && let Some(seg) = p.path.segments.last()
    {
        return seg.ident.to_string();
    }
    "error".to_string()
}

/// The `E` type of a `Result<T, E>` return, if the outer path is `Result` with
/// two generic arguments.
fn result_err_type(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    let types: Vec<&Type> = args
        .args
        .iter()
        .filter_map(|a| match a {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        })
        .collect();
    types.get(1).copied()
}

/// The `T` type of a `Result<T, E>` return, if inspectable.
fn result_ok_type(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

/// True when `ty` is an outer `Option<..>`.
fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Option"))
}

/// How an operation's return type is mapped to a CTSC completion event.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReturnKind {
    /// `()` / absent → no completion event.
    Unit,
    /// `Result<T, E>` → `result` (Ok) / `error` (Err).
    Result,
    /// `Result<Option<T>, E>` → `result` (Ok(Some)) / `empty` (Ok(None)) /
    /// `error` (Err). A static peel of the nested `Option`.
    ResultOption,
    /// `Option<T>` → `result` (Some) / `empty` (None).
    Option,
    /// Anything else → `result`.
    Other,
}

/// Classify a return type by the outermost path segment (`Result`/`Option`),
/// treating `()`/absent as `Unit` and everything else as `Other`. A
/// `Result<Option<T>, E>` is recognized as [`ReturnKind::ResultOption`].
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
                Some("Result") => {
                    if result_ok_type(t).is_some_and(is_option) {
                        ReturnKind::ResultOption
                    } else {
                        ReturnKind::Result
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_component_is_rejected() {
        // A bare `#[watch_operation]` (empty args) must fail to parse.
        syn::parse_str::<OperationArgs>("").unwrap_err();
    }

    #[test]
    fn empty_component_is_rejected() {
        syn::parse_str::<OperationArgs>(r#"component = """#).unwrap_err();
    }

    #[test]
    fn non_component_key_is_rejected() {
        syn::parse_str::<OperationArgs>(r#"spec = "x""#).unwrap_err();
    }

    #[test]
    fn component_is_parsed() {
        assert_eq!(
            syn::parse_str::<OperationArgs>(r#"component = "comp.app""#)
                .unwrap()
                .component,
            "comp.app"
        );
    }

    #[test]
    fn dep_name_only_inherits_component() {
        let args = syn::parse_str::<DepArgs>(r#""parse""#).unwrap();
        assert_eq!(args.name, "parse");
        assert_eq!(args.component, None);
    }

    #[test]
    fn dep_component_override_is_parsed() {
        let args = syn::parse_str::<DepArgs>(r#""parse", component = "other""#).unwrap();
        assert_eq!(args.name, "parse");
        assert_eq!(args.component.as_deref(), Some("other"));
    }

    #[test]
    fn result_error_name_uses_last_segment() {
        let out: ReturnType = syn::parse_str("-> Result<i64, std::num::ParseIntError>").unwrap();
        assert_eq!(result_error_name(&out), "ParseIntError");
        let out: ReturnType = syn::parse_str("-> Result<i64, String>").unwrap();
        assert_eq!(result_error_name(&out), "String");
    }

    #[test]
    fn classify_recognizes_result_option() {
        let out: ReturnType = syn::parse_str("-> Result<Option<i64>, String>").unwrap();
        assert!(classify_return(&out) == ReturnKind::ResultOption);
        let out: ReturnType = syn::parse_str("-> Result<i64, String>").unwrap();
        assert!(classify_return(&out) == ReturnKind::Result);
    }
}
