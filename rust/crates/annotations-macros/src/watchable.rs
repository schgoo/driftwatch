//! The `#[derive(Watchable)]` derive and its `#[watchable]` helper attribute.
//!
//! For a struct, emits [`Watchable`], [`ToValue`], and [`WatchableStruct`] impls
//! plus a `TypeMeta` registration; only fields tagged `#[watchable]` (honoring
//! `#[watchable(name = "…")]`) participate. For an enum, emits [`Watchable`] and
//! [`ToValue`] impls that match each variant (unit/tuple emit just the variant
//! tag; named variants emit their fields) plus one `VariantMeta` per variant.
//!
//! [`Watchable`]: runtime::Watchable
//! [`ToValue`]: runtime::ToValue
//! [`WatchableStruct`]: runtime::WatchableStruct

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, LitStr, Meta, parse_macro_input};

use crate::shared::{component_tokens, rt, sanitize_ident};

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let rt = rt();
    let component = component_tokens();
    let (impl_g, ty_g, where_c) = input.generics.split_for_impl();

    if let Data::Enum(data_enum) = &input.data {
        return expand_enum(name, data_enum, &impl_g, &ty_g, where_c, &component, &rt);
    }

    // --- Struct: emit each field tagged `#[watchable]` (opt-in). ---
    let mut emits = Vec::new();
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
            emits.push(quote! {
                let __dw_name = match __dw_prefix {
                    ::std::option::Option::Some(p) => ::std::format!("{}.{}", p, #fname),
                    ::std::option::Option::None => #fname.to_string(),
                };
                #rt::emit_event_v(&__dw_name, #rt::ToValue::to_value(&self.#id));
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
        impl #impl_g #rt::Watchable for #name #ty_g #where_c {
            fn emit_fields(&self, __dw_prefix: ::std::option::Option<&str>) {
                #(#emits)*
            }
        }
        impl #impl_g #rt::ToValue for #name #ty_g #where_c {
            fn to_value(&self) -> #rt::Value {
                let mut __dw_m = ::std::collections::BTreeMap::new();
                #(#to_value_inserts)*
                #rt::Value::Map(__dw_m)
            }
        }
        impl #impl_g #rt::WatchableStruct for #name #ty_g #where_c {}
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
/// The emit arm, `to_value` arm, and `VariantMeta` for one enum variant.
struct VariantParts {
    arm: TokenStream2,
    to_value_arm: TokenStream2,
    meta: TokenStream2,
}

fn build_variant(name: &Ident, variant: &syn::Variant, rt: &TokenStream2) -> VariantParts {
    let vname = &variant.ident;
    let vname_str = vname.to_string();
    match &variant.fields {
        Fields::Unit => VariantParts {
            meta: quote! { #rt::VariantMeta { name: #vname_str, fields: &[] } },
            arm: quote! {
                #name::#vname => {
                    #rt::emit_event_v(&__dw_base, #rt::Value::String(#vname_str.to_string()));
                }
            },
            to_value_arm: quote! {
                #name::#vname => {
                    let mut __dw_outer = ::std::collections::BTreeMap::new();
                    __dw_outer.insert(#vname_str.to_string(), #rt::Value::Map(::std::collections::BTreeMap::new()));
                    #rt::Value::Map(__dw_outer)
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
                arm: quote! {
                    #name::#vname { #(#field_idents),* } => {
                        #rt::emit_event_v(&__dw_base, #rt::Value::String(#vname_str.to_string()));
                        #(
                            #rt::emit_event_v(
                                &::std::format!("{}.{}", __dw_base, #field_strs),
                                #rt::ToValue::to_value(#field_idents),
                            );
                        )*
                    }
                },
                to_value_arm: quote! {
                    #name::#vname { #(#field_idents),* } => {
                        let mut __dw_inner = ::std::collections::BTreeMap::new();
                        #(
                            __dw_inner.insert(#field_strs.to_string(), #rt::ToValue::to_value(#field_idents));
                        )*
                        let mut __dw_outer = ::std::collections::BTreeMap::new();
                        __dw_outer.insert(#vname_str.to_string(), #rt::Value::Map(__dw_inner));
                        #rt::Value::Map(__dw_outer)
                    }
                },
            }
        }
        Fields::Unnamed(_) => VariantParts {
            meta: quote! { #rt::VariantMeta { name: #vname_str, fields: &[] } },
            arm: quote! {
                #name::#vname(..) => {
                    #rt::emit_event_v(&__dw_base, #rt::Value::String(#vname_str.to_string()));
                }
            },
            to_value_arm: quote! {
                #name::#vname(..) => {
                    let mut __dw_outer = ::std::collections::BTreeMap::new();
                    __dw_outer.insert(#vname_str.to_string(), #rt::Value::Map(::std::collections::BTreeMap::new()));
                    #rt::Value::Map(__dw_outer)
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
    let enum_name_lower = name.to_string().to_lowercase();
    let mut arms: Vec<TokenStream2> = Vec::new();
    let mut to_value_arms: Vec<TokenStream2> = Vec::new();
    let mut variant_metas: Vec<TokenStream2> = Vec::new();

    for variant in &data_enum.variants {
        let parts = build_variant(name, variant, rt);
        arms.push(parts.arm);
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
        impl #impl_g #rt::Watchable for #name #ty_g #where_c {
            fn emit_fields(&self, __dw_prefix: ::std::option::Option<&str>) {
                let __dw_base: ::std::string::String = match __dw_prefix {
                    ::std::option::Option::Some(p) => p.to_string(),
                    ::std::option::Option::None => #enum_name_lower.to_string(),
                };
                match self {
                    #(#arms)*
                }
            }
        }
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
