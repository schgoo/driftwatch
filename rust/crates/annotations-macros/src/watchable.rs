//! The `#[derive(Watchable)]` derive and its `#[watchable]` helper attribute.
//!
//! For a struct, emits a [`ToValue`] impl (a `Value::Map` of the
//! `#[watchable]`-tagged fields, honoring `#[watchable(name = "…")]`) plus a
//! `TypeMeta` registration. For an enum, emits a [`ToValue`] impl matching each
//! variant (unit/tuple → `Value::variant_unit`; named → a `Value::Variant`
//! carrying a field map) plus one `VariantMeta` per variant.
//!
//! The derive produces value **decomposition** only — there is no per-field
//! event emission. Field-mutation and inline observations are emitted directly
//! by the operation body/`watch_point!` as `conformance.observation` events; a
//! struct/enum return decomposes to a single `conformance.result` value via this
//! `ToValue` impl (CTSC has no per-field return decomposition), and an error
//! decomposes through the same impl in the `ValueEmit` ladder.
//!
//! [`ToValue`]: runtime::ToValue

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, LitStr, Meta, parse_macro_input};

use crate::shared::{rt, sanitize_ident};

/// The `component` field's token for a derived type: the annotated crate's
/// package name. The derive has no component surface, so a type inherits its
/// crate name (extraction groups by component; cross-language normalization is
/// the registry's job).
fn component_tokens() -> TokenStream2 {
    quote! { ::core::env!("CARGO_PKG_NAME") }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let rt = rt();
    let component = component_tokens();
    let (impl_g, ty_g, where_c) = input.generics.split_for_impl();

    if let Data::Enum(data_enum) = &input.data {
        return expand_enum(name, data_enum, &impl_g, &ty_g, where_c, &component, &rt);
    }

    // --- Struct: decompose each field tagged `#[watchable]` (opt-in). ---
    let mut to_value_inserts = Vec::new();
    let mut field_metas: Vec<TokenStream2> = Vec::new();
    if let Data::Struct(s) = &input.data {
        for field in &s.fields {
            let mut marked = false;
            let mut override_name: Option<String> = None;
            for a in &field.attrs {
                if !a.path().is_ident("watchable") {
                    continue;
                }
                marked = true;
                match watchable_field_name(a) {
                    Ok(Some(n)) => override_name = Some(n),
                    Ok(None) => {}
                    Err(e) => return e.to_compile_error().into(),
                }
            }
            if !marked {
                continue;
            }
            let Some(id) = &field.ident else { continue };
            let fname = override_name.unwrap_or_else(|| id.to_string());

            let fty = &field.ty;
            let fty_str = quote!(#fty).to_string();
            field_metas.push(quote! { (#fname, #fty_str) });

            to_value_inserts.push(quote! {
                __dw_m.insert(#fname.to_string(), #rt::ToValue::to_value(&self.#id));
            });
        }
    }

    let type_name_str = name.to_string();
    let reg = register_type_meta(
        &type_name_str,
        name,
        "struct",
        &field_metas,
        &[],
        &component,
        &rt,
    );
    quote! {
        impl #impl_g #rt::ToValue for #name #ty_g #where_c {
            fn to_value(&self) -> #rt::Value {
                let mut __dw_m = ::std::collections::BTreeMap::new();
                #(#to_value_inserts)*
                #rt::Value::Map(__dw_m)
            }
        }
        #reg
    }
    .into()
}

/// Parse a `#[watchable]` field attribute.
///
/// Returns `Ok(Some(name))` for a `#[watchable(name = "…")]` rename, `Ok(None)`
/// for a bare `#[watchable]` or empty `#[watchable()]`, and `Err` for a
/// malformed argument or unknown key — so the derive rejects those at compile
/// time instead of silently falling back to the field identifier.
fn watchable_field_name(attr: &Attribute) -> syn::Result<Option<String>> {
    // Only a list-form `#[watchable(...)]` carries arguments.
    if !matches!(attr.meta, Meta::List(_)) {
        return Ok(None);
    }
    let mut name = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("name") {
            let lit: LitStr = meta.value()?.parse()?;
            name = Some(lit.value());
            Ok(())
        } else {
            Err(meta.error("unknown `#[watchable]` argument; expected `name = \"...\"`"))
        }
    })?;
    Ok(name)
}
/// The `to_value` arm and `VariantMeta` for one enum variant.
struct VariantParts {
    to_value_arm: TokenStream2,
    meta: TokenStream2,
}

fn build_variant(name: &Ident, variant: &syn::Variant, rt: &TokenStream2) -> VariantParts {
    let vname = &variant.ident;
    let vname_str = vname.to_string();
    match &variant.fields {
        Fields::Unit => VariantParts {
            meta: quote! { #rt::VariantMeta { name: #vname_str, fields: &[] } },
            to_value_arm: quote! {
                #name::#vname => {
                    #rt::Value::variant_unit(#vname_str)
                }
            },
        },
        Fields::Named(named) => {
            let field_idents: Vec<&Ident> = named
                .named
                .iter()
                .filter_map(|f| f.ident.as_ref())
                .collect();
            let field_strs: Vec<String> = field_idents.iter().map(ToString::to_string).collect();
            let field_meta_entries: Vec<TokenStream2> = named
                .named
                .iter()
                .filter_map(|f| {
                    let id = f.ident.as_ref()?;
                    let ty = &f.ty;
                    let id_str = id.to_string();
                    let ty_str = quote!(#ty).to_string();
                    Some(quote! { (#id_str, #ty_str) })
                })
                .collect();
            VariantParts {
                meta: quote! { #rt::VariantMeta { name: #vname_str, fields: &[#(#field_meta_entries),*] } },
                to_value_arm: quote! {
                    #name::#vname { #(#field_idents),* } => {
                        let mut __dw_inner = ::std::collections::BTreeMap::new();
                        #(
                            __dw_inner.insert(#field_strs.to_string(), #rt::ToValue::to_value(#field_idents));
                        )*
                        #rt::Value::Variant {
                            tag: #vname_str.to_string(),
                            value: ::std::boxed::Box::new(#rt::Value::Map(__dw_inner)),
                        }
                    }
                },
            }
        }
        Fields::Unnamed(_) => VariantParts {
            meta: quote! { #rt::VariantMeta { name: #vname_str, fields: &[] } },
            to_value_arm: quote! {
                #name::#vname(..) => {
                    #rt::Value::variant_unit(#vname_str)
                }
            },
        },
    }
}

fn expand_enum(
    name: &Ident,
    data_enum: &syn::DataEnum,
    impl_g: &syn::ImplGenerics<'_>,
    ty_g: &syn::TypeGenerics<'_>,
    where_c: Option<&syn::WhereClause>,
    component: &TokenStream2,
    rt: &TokenStream2,
) -> TokenStream {
    let mut to_value_arms: Vec<TokenStream2> = Vec::new();
    let mut variant_metas: Vec<TokenStream2> = Vec::new();

    for variant in &data_enum.variants {
        let parts = build_variant(name, variant, rt);
        to_value_arms.push(parts.to_value_arm);
        variant_metas.push(parts.meta);
    }

    let type_name_str = name.to_string();
    let reg = register_type_meta(
        &type_name_str,
        name,
        "enum",
        &[],
        &variant_metas,
        component,
        rt,
    );
    quote! {
        impl #impl_g #rt::ToValue for #name #ty_g #where_c {
            fn to_value(&self) -> #rt::Value {
                match self {
                    #(#to_value_arms)*
                }
            }
        }
        #reg
    }
    .into()
}

/// Build the `DRIFTWATCH_TYPES` registration for a `Watchable` type, wrapped in
/// a named `const` so it compiles wherever the type is declared.
#[allow(
    clippy::too_many_arguments,
    reason = "one registration site; splitting would obscure the flat metadata"
)]
fn register_type_meta(
    name_str: &str,
    name: &Ident,
    kind: &str,
    fields: &[TokenStream2],
    variants: &[TokenStream2],
    component: &TokenStream2,
    rt: &TokenStream2,
) -> TokenStream2 {
    let suffix = sanitize_ident(name_str);
    let const_ident = Ident::new(&format!("_DRIFTWATCH_TYPE_REG_{suffix}"), name.span());
    let static_ident = Ident::new(&format!("_DRIFTWATCH_TYPE_S_{suffix}"), name.span());
    quote! {
        #[allow(dead_code, non_upper_case_globals, reason = "macro-generated registry scaffolding")]
        const #const_ident: () = {
            #[#rt::linkme::distributed_slice(#rt::DRIFTWATCH_TYPES)]
            #[linkme(crate = #rt::linkme)]
            static #static_ident: #rt::TypeMeta = #rt::TypeMeta {
                name: #name_str,
                module_path: ::core::module_path!(),
                kind: #kind,
                fields: &[#(#fields),*],
                variants: &[#(#variants),*],
                component: #component,
            };
        };
    }
}
#[cfg(test)]
mod tests {
    use super::watchable_field_name;
    use quote::quote;
    use syn::{Attribute, parse::Parser};

    fn attr(tokens: proc_macro2::TokenStream) -> Attribute {
        Attribute::parse_outer
            .parse2(tokens)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn bare_watchable_has_no_override() {
        assert_eq!(
            watchable_field_name(&attr(quote!(#[watchable]))).unwrap(),
            None
        );
    }

    #[test]
    fn empty_list_has_no_override() {
        assert_eq!(
            watchable_field_name(&attr(quote!(#[watchable()]))).unwrap(),
            None
        );
    }

    #[test]
    fn name_override_is_parsed() {
        assert_eq!(
            watchable_field_name(&attr(quote!(#[watchable(name = "display")]))).unwrap(),
            Some("display".to_string())
        );
    }

    #[test]
    fn unknown_key_is_rejected() {
        watchable_field_name(&attr(quote!(#[watchable(bogus)]))).unwrap_err();
    }

    #[test]
    fn non_string_name_is_rejected() {
        watchable_field_name(&attr(quote!(#[watchable(name = 42)]))).unwrap_err();
    }

    #[test]
    fn missing_name_value_is_rejected() {
        watchable_field_name(&attr(quote!(#[watchable(name)]))).unwrap_err();
    }
}
