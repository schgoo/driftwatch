//! Corpus test: vendored CTSC Registry 0.1 fixtures drive parse + validate.
//!
//! Valid fixtures must parse and produce no violations; each invalid fixture
//! must produce its specific [`ViolationKind`]. Fixtures are copied verbatim
//! from the CTSC registry corpus (`docs/ctsc/corpus/registry/`).

use std::path::Path;

use contract::{RegistryDocument, ViolationKind, validate};

fn load(rel: &str) -> RegistryDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(rel);
    let json =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    RegistryDocument::parse(&json).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn order_pricing_registry_when_validated_is_clean() {
    let doc = load("valid/order-pricing.registry.json");
    assert_eq!(validate(&doc), Vec::new());
}

#[test]
fn values_registry_when_validated_is_clean() {
    let doc = load("valid/values.registry.json");
    assert_eq!(validate(&doc), Vec::new());
}

#[test]
fn duplicate_operation_name_when_validated_is_duplicate_name() {
    let doc = load("invalid/duplicate-operation.registry.json");
    let violations = validate(&doc);
    assert!(
        violations.iter().any(|v| matches!(
            &v.kind,
            ViolationKind::DuplicateName { scope: "operation name", name } if name == "read"
        )),
        "expected DuplicateName for operation `read`, got {violations:?}"
    );
}

#[test]
fn empty_outcome_without_result_when_validated_is_empty_without_result() {
    let doc = load("invalid/empty-without-result.registry.json");
    let violations = validate(&doc);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v.kind, ViolationKind::EmptyWithoutResult)),
        "expected EmptyWithoutResult, got {violations:?}"
    );
}

#[test]
fn same_component_named_ref_when_undeclared_is_unresolved_type() {
    let doc = load("invalid/unresolved-type.registry.json");
    let violations = validate(&doc);
    assert!(
        violations.iter().any(|v| matches!(
            &v.kind,
            ViolationKind::UnresolvedType { name } if name == "MissingType"
        )),
        "expected UnresolvedType for `MissingType`, got {violations:?}"
    );
}

#[test]
fn cross_component_ref_when_dependency_absent_is_missing_dependency() {
    let doc = load("invalid/missing-dependency.registry.json");
    let violations = validate(&doc);
    assert!(
        violations.iter().any(|v| matches!(
            &v.kind,
            ViolationKind::MissingDependency { component_id } if component_id == "example.tax"
        )),
        "expected MissingDependency for `example.tax`, got {violations:?}"
    );
}
