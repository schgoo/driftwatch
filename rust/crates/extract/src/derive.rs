//! Derive a [`contract::RegistryDocument`] from the runtime discovery metadata
//! (registry.md §2–§5).
//!
//! [`derive`] maps the typed `runtime::OpMeta`/`runtime::TypeMeta` slices — the
//! link-time registry's operation and `Watchable`-type descriptions — into the
//! CTSC registry contract model. It reads the typed slices directly rather than
//! `runtime::discovery_json`, so `runtime` stays untouched and callers (and
//! tests) can hand it fixture slices.
//!
//! ## Component grouping
//!
//! Operations and types are grouped by their `OpMeta.component`/
//! `TypeMeta.component` tag, emitting one [`contract::Component`] per distinct
//! tag under `components[]` (a component is never top-level; even a lone
//! component nests here). Components are sorted by `id`, and within each
//! component operations and types are sorted by `name`, so the derived
//! document is byte-reproducible regardless of the (platform-dependent)
//! link-time registry order. Each derived component gets an empty
//! `dependencies` list:
//! deriving cross-component `dependencies[]` from cross-component `Named`
//! references is deferred, so a reference that crosses a component boundary
//! surfaces as a `contract::validate` dangling-reference violation (§8) until
//! that slice lands.
//!
//! `registryId`/`version` come solely from the [`RegistryIdentity`] parameter
//! (config/env resolution is roadmap #10b); a component's `id` comes solely
//! from its discovery `component` tag.

use std::collections::BTreeSet;

use contract::{
    Component, FORMAT, FORMAT_VERSION, Field, NamedType, NamedValue, Operation, RegistryDocument,
    TypeRef, Variant,
};
use runtime::{OpMeta, TypeMeta, VariantMeta};

use crate::identity::RegistryIdentity;
use crate::return_kind::classify_return;
use crate::type_ref::parse_type_ref;

/// The component id used only when the registry carries no operations or types,
/// so the document still declares the CTSC-required one-or-more components.
const DEFAULT_COMPONENT: &str = "default";

/// Derive a CTSC registry document from runtime discovery metadata.
///
/// Setups (`OpMeta::is_setup`) are wiring helpers, not contract operations, so
/// they are excluded from the derived operations (and from component grouping).
#[must_use]
pub fn derive(ops: &[OpMeta], types: &[TypeMeta], identity: RegistryIdentity) -> RegistryDocument {
    let known: BTreeSet<&str> = types.iter().map(|t| t.name).collect();

    let mut components: Vec<Component> = component_ids(ops, types)
        .into_iter()
        .map(|cid| derive_component(cid, ops, types, &known))
        .collect();
    if components.is_empty() {
        components.push(Component {
            id: DEFAULT_COMPONENT.to_string(),
            description: None,
            dependencies: Vec::new(),
            operations: Vec::new(),
            types: Vec::new(),
        });
    }

    RegistryDocument {
        format: FORMAT.to_string(),
        format_version: FORMAT_VERSION.to_string(),
        registry_id: identity.registry_id,
        version: identity.version,
        description: None,
        imports: Vec::new(),
        components,
    }
}

/// The distinct component tags across the (non-setup) operations and types,
/// sorted by tag so the derived document is order-stable regardless of the
/// (platform-dependent) link-time registry order.
fn component_ids<'a>(ops: &'a [OpMeta], types: &'a [TypeMeta]) -> Vec<&'a str> {
    ops.iter()
        .filter(|op| !op.is_setup)
        .map(|op| op.component)
        .chain(types.iter().map(|t| t.component))
        .collect::<BTreeSet<&str>>()
        .into_iter()
        .collect()
}

/// Build the [`Component`] for one component tag from the operations and types
/// that carry it.
fn derive_component(
    cid: &str,
    ops: &[OpMeta],
    types: &[TypeMeta],
    known: &BTreeSet<&str>,
) -> Component {
    let mut operations: Vec<Operation> = ops
        .iter()
        .filter(|op| !op.is_setup && op.component == cid)
        .map(|op| derive_operation(op, known))
        .collect();
    operations.sort_by(|a, b| a.name.cmp(&b.name));

    let mut named_types: Vec<NamedType> = types
        .iter()
        .filter(|t| t.component == cid)
        .map(|t| derive_named_type(t, known))
        .collect();
    named_types.sort_by(|a, b| a.name().cmp(b.name()));

    Component {
        id: cid.to_string(),
        description: None,
        dependencies: Vec::new(),
        operations,
        types: named_types,
    }
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
    use contract::validate;
    use runtime::FieldMeta;

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
