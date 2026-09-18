//! Derive a [`contract::RegistryDocument`] from the runtime discovery metadata
//! (registry.md §2–§5).
//!
//! [`derive`] maps the typed `runtime::OpMeta`/`runtime::TypeMeta` slices — the
//! link-time registry's operation and `Watchable`-type descriptions — into the
//! CTSC registry contract model. It reads the typed slices directly rather than
//! `runtime::discovery_json`, so `runtime` stays untouched and callers (and
//! tests) can hand it fixture slices.
//!
//! This is the front-half (string lowering) of derivation: it lowers each
//! `OpMeta`/`TypeMeta` into a resolved contract [`contract::Operation`]/
//! [`contract::NamedType`] (via [`parse_type_ref`]/[`classify_return`]), builds
//! the [`contract::ResolvedOperation`]/[`contract::ResolvedType`] IR, then hands
//! off to [`contract::assemble`] for component grouping, sorting, the
//! default-component fallback, and the document envelope. Keeping the two seams
//! apart lets a future static resolver feed already-resolved contract types
//! straight into [`contract::assemble`] without routing back through the string
//! parsers here.
//!
//! `registryId`/`version` come solely from the [`RegistryIdentity`] parameter
//! (config/env resolution is roadmap #10b); a component's `id` comes solely
//! from its discovery `component` tag.

use std::collections::BTreeSet;

use contract::{
    Field, NamedType, NamedValue, Operation, RegistryDocument, ResolvedOperation, ResolvedType,
    TypeRef, Variant,
};
use runtime::{OpMeta, TypeMeta, VariantMeta};

use crate::identity::RegistryIdentity;
use crate::return_kind::classify_return;
use crate::type_ref::parse_type_ref;

/// Derive a CTSC registry document from runtime discovery metadata.
///
/// Setups (`OpMeta::is_setup`) are wiring helpers, not contract operations, so
/// they are excluded from the derived operations (and from component grouping).
/// The setup exclusion is enforced by [`contract::assemble`] from the
/// `is_setup` flag carried on each [`ResolvedOperation`].
#[must_use]
pub fn derive(ops: &[OpMeta], types: &[TypeMeta], identity: RegistryIdentity) -> RegistryDocument {
    let known: BTreeSet<&str> = types.iter().map(|t| t.name).collect();

    let resolved_ops: Vec<ResolvedOperation> = ops
        .iter()
        .map(|op| ResolvedOperation {
            component: op.component.to_string(),
            is_setup: op.is_setup,
            operation: derive_operation(op, &known),
        })
        .collect();

    let resolved_types: Vec<ResolvedType> = types
        .iter()
        .map(|t| ResolvedType {
            component: t.component.to_string(),
            named_type: derive_named_type(t, &known),
        })
        .collect();

    contract::assemble(
        &resolved_ops,
        &resolved_types,
        identity.registry_id,
        identity.version,
    )
}

/// Lower one `OpMeta` into a contract [`Operation`]: parameters become inputs;
/// the return type becomes completion outcomes; observations are deferred (not
/// statically knowable from the signature).
fn derive_operation(op: &OpMeta, known: &BTreeSet<&str>) -> Operation {
    Operation {
        name: op.name.to_string(),
        description: None,
        inputs: op
            .params
            .iter()
            .map(|(name, ty)| NamedValue {
                name: (*name).to_string(),
                ty: parse_type_ref(ty, known),
                description: None,
            })
            .collect(),
        observations: Vec::new(),
        outcomes: classify_return(op.return_type, known),
    }
}

/// Lower one `TypeMeta` into a contract [`NamedType`]: `"struct"` → record,
/// anything else (`"enum"`) → tagged union.
fn derive_named_type(ty: &TypeMeta, known: &BTreeSet<&str>) -> NamedType {
    if ty.kind == "struct" {
        NamedType::Record {
            name: ty.name.to_string(),
            fields: ty.fields.iter().map(|f| derive_field(f, known)).collect(),
            description: None,
        }
    } else {
        NamedType::TaggedUnion {
            name: ty.name.to_string(),
            variants: ty
                .variants
                .iter()
                .map(|v| derive_variant(v, known))
                .collect(),
            description: None,
        }
    }
}

/// Lower one named field `(name, type)` pair into a record [`Field`].
fn derive_field(field: &(&str, &str), known: &BTreeSet<&str>) -> Field {
    let (name, ty) = *field;
    Field {
        name: name.to_string(),
        ty: parse_type_ref(ty, known),
        description: None,
    }
}

/// Lower one `VariantMeta` into a tagged-union [`Variant`]. A variant with named
/// fields carries an inline record payload; a tuple/unit variant (empty field
/// list) carries no payload.
fn derive_variant(variant: &VariantMeta, known: &BTreeSet<&str>) -> Variant {
    let payload = if variant.fields.is_empty() {
        None
    } else {
        Some(TypeRef::Record {
            fields: variant
                .fields
                .iter()
                .map(|f| derive_field(f, known))
                .collect(),
        })
    };
    Variant {
        name: variant.name.to_string(),
        payload,
        description: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::{FORMAT, FORMAT_VERSION, validate};
    use runtime::FieldMeta;

    /// Mirrors the private `contract::assemble::DEFAULT_COMPONENT` fallback id,
    /// so the empty-registry adapter test keeps asserting the same value.
    const DEFAULT_COMPONENT: &str = "default";

    fn identity() -> RegistryIdentity {
        RegistryIdentity {
            registry_id: "urn:ctsc:registry:example:1".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    const USER_FIELDS: &[FieldMeta] = &[("id", "u64"), ("name", "String")];

    const CHARGE_PARAMS: &[FieldMeta] = &[("user", "User"), ("cents", "u64")];

    fn ops() -> Vec<OpMeta> {
        vec![
            OpMeta {
                name: "charge",
                module_path: "billing::pay",
                fn_name: "charge",
                is_setup: false,
                is_async: false,
                params: CHARGE_PARAMS,
                return_type: "Result<Receipt, ChargeError>",
                fills: "",
                component: "billing",
            },
            OpMeta {
                name: "find_user",
                module_path: "billing::users",
                fn_name: "find_user",
                is_setup: false,
                is_async: false,
                params: &[("id", "u64")],
                return_type: "Option<User>",
                fills: "",
                component: "billing",
            },
            // A setup must not appear as an operation.
            OpMeta {
                name: "seed",
                module_path: "billing::seed",
                fn_name: "seed",
                is_setup: true,
                is_async: false,
                params: &[],
                return_type: "User",
                fills: "user",
                component: "billing",
            },
        ]
    }

    fn types() -> Vec<TypeMeta> {
        vec![
            TypeMeta {
                name: "User",
                module_path: "billing::model",
                kind: "struct",
                fields: USER_FIELDS,
                variants: &[],
                component: "billing",
            },
            TypeMeta {
                name: "Receipt",
                module_path: "billing::model",
                kind: "struct",
                fields: &[("amount", "u64")],
                variants: &[],
                component: "billing",
            },
            TypeMeta {
                name: "ChargeError",
                module_path: "billing::model",
                kind: "enum",
                fields: &[],
                variants: &[
                    VariantMeta {
                        name: "Declined",
                        fields: &[],
                    },
                    VariantMeta {
                        name: "OverLimit",
                        fields: &[("limit", "u64")],
                    },
                ],
                component: "billing",
            },
        ]
    }

    #[test]
    fn derived_document_validates_cleanly() {
        let doc = derive(&ops(), &types(), identity());
        assert_eq!(validate(&doc), Vec::new());
    }

    #[test]
    fn derived_document_stamps_format_and_identity() {
        let doc = derive(&ops(), &types(), identity());
        assert_eq!(doc.format, FORMAT);
        assert_eq!(doc.format_version, FORMAT_VERSION);
        assert_eq!(doc.registry_id, "urn:ctsc:registry:example:1");
        assert_eq!(doc.version, "1.0.0");
        assert_eq!(doc.components.len(), 1);
        assert_eq!(doc.components[0].id, "billing");
    }

    #[test]
    fn setups_are_excluded_from_operations() {
        let doc = derive(&ops(), &types(), identity());
        let names: Vec<&str> = doc.components[0]
            .operations
            .iter()
            .map(|o| o.name.as_str())
            .collect();
        assert_eq!(names, ["charge", "find_user"]);
    }

    #[test]
    fn document_round_trips_through_json() {
        let doc = derive(&ops(), &types(), identity());
        let json = serde_json::to_string(&doc).expect("serializes");
        let reparsed = RegistryDocument::parse(&json).expect("reparses");
        assert_eq!(reparsed, doc);
    }

    #[test]
    fn empty_registry_yields_default_component() {
        let doc = derive(&[], &[], identity());
        assert_eq!(doc.components[0].id, DEFAULT_COMPONENT);
        assert_eq!(validate(&doc), Vec::new());
    }

    #[test]
    fn undeclared_named_reference_is_flagged_as_unresolved() {
        // `charge` references `Receipt` and `ChargeError`, which we do NOT
        // declare, so validate must report the §8 dangling reference.
        let doc = derive(&ops(), &[], identity());
        let violations = validate(&doc);
        assert!(
            violations.iter().any(|v| matches!(
                &v.kind,
                contract::ViolationKind::UnresolvedType { name } if name == "Receipt"
            )),
            "expected an unresolved-type violation, got {violations:?}"
        );
    }

    #[test]
    fn distinct_tags_group_into_one_component_each() {
        // Two components, each self-contained (no cross-component refs), so the
        // derived document still validates cleanly.
        let ops = [
            OpMeta {
                name: "charge",
                module_path: "billing::pay",
                fn_name: "charge",
                is_setup: false,
                is_async: false,
                params: &[("cents", "u64")],
                return_type: "u64",
                fills: "",
                component: "billing",
            },
            OpMeta {
                name: "notify",
                module_path: "mail::send",
                fn_name: "notify",
                is_setup: false,
                is_async: false,
                params: &[("to", "String")],
                return_type: "bool",
                fills: "",
                component: "mail",
            },
        ];
        let types = [TypeMeta {
            name: "Envelope",
            module_path: "mail::model",
            kind: "struct",
            fields: &[("subject", "String")],
            variants: &[],
            component: "mail",
        }];

        let doc = derive(&ops, &types, identity());

        let ids: Vec<&str> = doc.components.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["billing", "mail"]);

        let billing = &doc.components[0];
        assert_eq!(billing.operations.len(), 1);
        assert_eq!(billing.operations[0].name, "charge");
        assert!(billing.types.is_empty());

        let mail = &doc.components[1];
        assert_eq!(mail.operations.len(), 1);
        assert_eq!(mail.operations[0].name, "notify");
        assert_eq!(mail.types.len(), 1);
        assert_eq!(mail.types[0].name(), "Envelope");

        for component in &doc.components {
            assert!(component.dependencies.is_empty());
        }
        assert_eq!(validate(&doc), Vec::new());
    }
}
