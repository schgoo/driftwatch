//! Cross-item registry validation (registry.md §4.1, §7, §8, §10).
//!
//! JSON Schema (`ctsc-registry-0.2.schema.json`) validates document *shape*;
//! this module enforces the semantic rules the schema cannot express: name
//! uniqueness, outcome constraints, and intra-document named-type resolution.
//!
//! Loading imported documents and verifying their digests (§6) — and the
//! cross-document type resolution that depends on them — is deferred to the
//! Linked-validation slice; a cross-*document* reference is checked here only
//! structurally, by requiring a matching component dependency (§7).

use std::collections::BTreeSet;
use std::fmt;

use crate::component::Component;
use crate::document::{FORMAT, FORMAT_VERSION, RegistryDocument};
use crate::named_type::NamedType;
use crate::operation::Operation;
use crate::type_ref::TypeRef;

/// A single registry-validation failure, located within the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// A human-readable location within the document (component/operation/type).
    pub location: String,
    /// What rule the document broke.
    pub kind: ViolationKind,
}

/// The distinct ways a registry document can be semantically invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    /// A `format` / `formatVersion` value this crate does not model (§2).
    WrongFormat {
        /// The offending field name.
        field: &'static str,
        /// The value found in the document.
        found: String,
        /// The value this crate expects.
        expected: &'static str,
    },
    /// A name that must be unique in its scope appears more than once (§10).
    DuplicateName {
        /// The scope whose names must be unique (e.g. `"operation name"`).
        scope: &'static str,
        /// The repeated name.
        name: String,
    },
    /// `empty: true` without a `result` outcome (§4.1).
    EmptyWithoutResult,
    /// A same-component named reference with no matching type declaration (§8).
    UnresolvedType {
        /// The unresolved type name.
        name: String,
    },
    /// A cross-component named reference with no matching dependency (§7).
    MissingDependency {
        /// The referenced component id lacking a dependency.
        component_id: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.kind)
    }
}

impl fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViolationKind::WrongFormat {
                field,
                found,
                expected,
            } => write!(f, "{field} is {found:?}, expected {expected:?}"),
            ViolationKind::DuplicateName { scope, name } => {
                write!(f, "duplicate {scope} {name:?}")
            }
            ViolationKind::EmptyWithoutResult => {
                write!(f, "outcomes declare `empty` without a `result`")
            }
            ViolationKind::UnresolvedType { name } => {
                write!(f, "named type {name:?} is not declared in this component")
            }
            ViolationKind::MissingDependency { component_id } => write!(
                f,
                "reference to component {component_id:?} without a matching dependency"
            ),
        }
    }
}

/// Validate a registry document against the cross-item rules of registry.md.
///
/// Returns every [`Violation`] found (validation does not stop at the first);
/// an empty vector means the document is valid Registry input.
#[must_use]
pub fn validate(doc: &RegistryDocument) -> Vec<Violation> {
    let mut out = Vec::new();

    if doc.format != FORMAT {
        out.push(Violation {
            location: "document".to_string(),
            kind: ViolationKind::WrongFormat {
                field: "format",
                found: doc.format.clone(),
                expected: FORMAT,
            },
        });
    }
    if doc.format_version != FORMAT_VERSION {
        out.push(Violation {
            location: "document".to_string(),
            kind: ViolationKind::WrongFormat {
                field: "formatVersion",
                found: doc.format_version.clone(),
                expected: FORMAT_VERSION,
            },
        });
    }

    for name in duplicates(doc.imports.iter().map(|i| i.registry_id.as_str())) {
        out.push(Violation {
            location: "document".to_string(),
            kind: ViolationKind::DuplicateName {
                scope: "import registryId",
                name,
            },
        });
    }
    for name in duplicates(doc.components.iter().map(|c| c.id.as_str())) {
        out.push(Violation {
            location: "document".to_string(),
            kind: ViolationKind::DuplicateName {
                scope: "component id",
                name,
            },
        });
    }

    for component in &doc.components {
        validate_component(component, &mut out);
    }

    out
}

fn validate_component(component: &Component, out: &mut Vec<Violation>) {
    let at = format!("component {:?}", component.id);

    for name in duplicates(
        component
            .dependencies
            .iter()
            .map(|d| d.component_id.as_str()),
    ) {
        out.push(Violation {
            location: at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "dependency componentId",
                name,
            },
        });
    }
    for name in duplicates(component.operations.iter().map(|o| o.name.as_str())) {
        out.push(Violation {
            location: at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "operation name",
                name,
            },
        });
    }
    for name in duplicates(component.types.iter().map(NamedType::name)) {
        out.push(Violation {
            location: at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "type name",
                name,
            },
        });
    }

    for named_type in &component.types {
        validate_named_type(
            named_type,
            &format!("{at} type {:?}", named_type.name()),
            out,
        );
    }

    let declared_types: BTreeSet<&str> = component.types.iter().map(NamedType::name).collect();
    let declared_deps: BTreeSet<&str> = component
        .dependencies
        .iter()
        .map(|d| d.component_id.as_str())
        .collect();

    for operation in &component.operations {
        validate_operation(operation, &at, &declared_types, &declared_deps, out);
    }
}

fn validate_operation(
    operation: &Operation,
    at: &str,
    declared_types: &BTreeSet<&str>,
    declared_deps: &BTreeSet<&str>,
    out: &mut Vec<Violation>,
) {
    let op_at = format!("{at} operation {:?}", operation.name);

    for name in duplicates(operation.inputs.iter().map(|i| i.name.as_str())) {
        out.push(Violation {
            location: op_at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "input name",
                name,
            },
        });
    }
    for name in duplicates(operation.observations.iter().map(|o| o.name.as_str())) {
        out.push(Violation {
            location: op_at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "observation name",
                name,
            },
        });
    }
    for name in duplicates(operation.outcomes.errors.iter().map(|e| e.name.as_str())) {
        out.push(Violation {
            location: op_at.clone(),
            kind: ViolationKind::DuplicateName {
                scope: "error name",
                name,
            },
        });
    }

    if operation.outcomes.empty && operation.outcomes.result.is_none() {
        out.push(Violation {
            location: op_at.clone(),
            kind: ViolationKind::EmptyWithoutResult,
        });
    }

    let mut refs = Vec::new();
    for input in &operation.inputs {
        walk_type_refs(&input.ty, &mut refs);
    }
    for observation in &operation.observations {
        walk_type_refs(&observation.ty, &mut refs);
    }
    if let Some(result) = &operation.outcomes.result {
        walk_type_refs(result, &mut refs);
    }
    for error in &operation.outcomes.errors {
        if let Some(ty) = &error.ty {
            walk_type_refs(ty, &mut refs);
        }
    }
    for type_ref in refs {
        resolve_named(type_ref, &op_at, declared_types, declared_deps, out);
        check_inline_uniqueness(type_ref, &op_at, out);
    }
}

fn validate_named_type(named_type: &NamedType, at: &str, out: &mut Vec<Violation>) {
    match named_type {
        NamedType::Record { fields, .. } => {
            for name in duplicates(fields.iter().map(|f| f.name.as_str())) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::DuplicateName {
                        scope: "field name",
                        name,
                    },
                });
            }
        }
        NamedType::TaggedUnion { variants, .. } => {
            for name in duplicates(variants.iter().map(|v| v.name.as_str())) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::DuplicateName {
                        scope: "variant name",
                        name,
                    },
                });
            }
        }
    }
}

/// Resolve one named reference: same-component names against the component's
/// declared types (§8); cross-component names against declared dependencies
/// (§7). Non-`named` refs are ignored.
fn resolve_named(
    type_ref: &TypeRef,
    at: &str,
    declared_types: &BTreeSet<&str>,
    declared_deps: &BTreeSet<&str>,
    out: &mut Vec<Violation>,
) {
    let TypeRef::Named {
        name, component_id, ..
    } = type_ref
    else {
        return;
    };
    match component_id {
        None => {
            if !declared_types.contains(name.as_str()) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::UnresolvedType { name: name.clone() },
                });
            }
        }
        Some(component_id) => {
            if !declared_deps.contains(component_id.as_str()) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::MissingDependency {
                        component_id: component_id.clone(),
                    },
                });
            }
        }
    }
}

/// Flag duplicate field/variant names in an inline record or tagged union.
fn check_inline_uniqueness(type_ref: &TypeRef, at: &str, out: &mut Vec<Violation>) {
    match type_ref {
        TypeRef::Record { fields } => {
            for name in duplicates(fields.iter().map(|f| f.name.as_str())) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::DuplicateName {
                        scope: "field name",
                        name,
                    },
                });
            }
        }
        TypeRef::TaggedUnion { variants } => {
            for name in duplicates(variants.iter().map(|v| v.name.as_str())) {
                out.push(Violation {
                    location: at.to_string(),
                    kind: ViolationKind::DuplicateName {
                        scope: "variant name",
                        name,
                    },
                });
            }
        }
        _ => {}
    }
}

/// Depth-first collect every `TypeRef` node reachable from `root` (inclusive).
fn walk_type_refs<'a>(root: &'a TypeRef, out: &mut Vec<&'a TypeRef>) {
    out.push(root);
    match root {
        TypeRef::List { items } | TypeRef::Set { items } => walk_type_refs(items, out),
        TypeRef::Optional { value } => walk_type_refs(value, out),
        TypeRef::Map { keys, values } => {
            walk_type_refs(keys, out);
            walk_type_refs(values, out);
        }
        TypeRef::Tuple { items } => {
            for item in items {
                walk_type_refs(item, out);
            }
        }
        TypeRef::Record { fields } => {
            for field in fields {
                walk_type_refs(&field.ty, out);
            }
        }
        TypeRef::TaggedUnion { variants } => {
            for variant in variants {
                if let Some(payload) = &variant.payload {
                    walk_type_refs(payload, out);
                }
            }
        }
        TypeRef::Primitive { .. } | TypeRef::Named { .. } => {}
    }
}

/// The values that appear more than once in `iter`, each reported once, in
/// sorted order for deterministic output.
fn duplicates<'a>(iter: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut repeated = BTreeSet::new();
    for name in iter {
        if !seen.insert(name) {
            repeated.insert(name);
        }
    }
    repeated.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use crate::RegistryDocument;

    use super::{ViolationKind, validate};

    fn parse(json: &str) -> RegistryDocument {
        RegistryDocument::parse(json).expect("fixture parses")
    }

    #[test]
    fn foreign_format_when_validated_is_wrong_format() {
        let doc = parse(
            r#"{
              "format": "not.ctsc",
              "formatVersion": "9.9.9",
              "registryId": "urn:ctsc:registry:x:1",
              "version": "1.0.0",
              "components": [{ "id": "x", "operations": [], "types": [] }]
            }"#,
        );
        let violations = validate(&doc);
        assert!(violations.iter().any(|v| matches!(
            &v.kind,
            ViolationKind::WrongFormat {
                field: "format",
                ..
            }
        )));
        assert!(violations.iter().any(|v| matches!(
            &v.kind,
            ViolationKind::WrongFormat {
                field: "formatVersion",
                ..
            }
        )));
    }

    #[test]
    fn inline_record_when_field_repeats_is_duplicate_name() {
        let doc = parse(
            r#"{
              "format": "ctsc.registry",
              "formatVersion": "0.2.0",
              "registryId": "urn:ctsc:registry:x:1",
              "version": "1.0.0",
              "components": [{
                "id": "x",
                "operations": [{
                  "name": "op",
                  "inputs": [{
                    "name": "arg",
                    "type": {
                      "kind": "record",
                      "fields": [
                        { "name": "dup", "type": { "kind": "primitive", "name": "i32" } },
                        { "name": "dup", "type": { "kind": "primitive", "name": "i32" } }
                      ]
                    }
                  }],
                  "observations": [],
                  "outcomes": {}
                }],
                "types": []
              }]
            }"#,
        );
        let violations = validate(&doc);
        assert!(
            violations.iter().any(|v| matches!(
                &v.kind,
                ViolationKind::DuplicateName { scope: "field name", name } if name == "dup"
            )),
            "expected inline field DuplicateName, got {violations:?}"
        );
    }

    #[test]
    fn canonical_document_when_validated_is_clean() {
        let doc = parse(
            r#"{
              "format": "ctsc.registry",
              "formatVersion": "0.2.0",
              "registryId": "urn:ctsc:registry:x:1",
              "version": "1.0.0",
              "components": [{ "id": "x", "operations": [], "types": [] }]
            }"#,
        );
        assert_eq!(validate(&doc), Vec::new());
    }
}
