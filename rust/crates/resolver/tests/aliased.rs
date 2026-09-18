//! Feature-gated alias-recovery proof for the static resolver.
//!
//! Points [`resolve`] at the in-crate `aliased` fixture and asserts the headline
//! value: rust-analyzer's real type inference recovers error channels that a
//! `type` alias (`io::Result`) or a foreign `Result` alias (`anyhow::Result`)
//! hides from the build-time string pipeline, and decomposes a
//! `#[derive(Watchable)]` enum into a tagged union.
//!
//! Gated on `ra`: without the feature, resolution is unavailable and there is
//! nothing to prove here.
#![cfg(feature = "ra")]

use std::path::PathBuf;

use contract::{NamedType, Operation, Primitive, ResolvedOperation, ResolvedType, TypeRef};
use resolver::{ResolveTarget, resolve};

/// The in-crate fixture crate the resolver is aimed at.
fn fixture_target() -> ResolveTarget {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("tests");
    dir.push("fixtures");
    dir.push("aliased");
    ResolveTarget::new(dir)
}

/// Look up a resolved operation by name.
fn operation<'a>(ops: &'a [ResolvedOperation], name: &str) -> &'a Operation {
    &ops.iter()
        .find(|op| op.operation.name == name)
        .unwrap_or_else(|| panic!("resolved operation `{name}` not found"))
        .operation
}

/// Assert `op` resolves to an `i64` result with exactly one name-only error.
fn assert_i64_named_error(op: &Operation, error_name: &str) {
    assert_eq!(
        op.outcomes.result,
        Some(TypeRef::Primitive {
            name: Primitive::I64
        }),
        "`{}` result is `i64`",
        op.name
    );
    assert_eq!(op.outcomes.errors.len(), 1, "`{}` has one error", op.name);
    assert_eq!(
        op.outcomes.errors[0].name, error_name,
        "`{}` error name",
        op.name
    );
    assert!(
        op.outcomes.errors[0].ty.is_none(),
        "the recovered error is name-only (v1)"
    );
}

#[test]
fn recovers_aliased_error_channels_and_decomposes_watchable_enum() {
    let resolved = resolve(&fixture_target()).expect("resolve the aliased fixture");

    // The `std::io::Result<i64>` alias resolves to `i64` + a recovered,
    // name-only `Error` (`io::Error`'s last path segment, producer-choice #4).
    assert_i64_named_error(operation(&resolved.operations, "first_byte"), "Error");

    // A second, distinct foreign crate: `anyhow::Error` collapses to the *same*
    // name-only `Error` — the spec-sanctioned collision.
    assert_i64_named_error(operation(&resolved.operations, "parse_amount"), "Error");

    // The `#[derive(Watchable)]` enum decomposes into a tagged union, tagged
    // with the owning crate's name (mirroring the runtime derive's
    // `CARGO_PKG_NAME` stamp) — NOT the declaring module path.
    let charge = resolved
        .types
        .iter()
        .find(|t: &&ResolvedType| t.named_type.name() == "ChargeError")
        .expect("ChargeError resolved");
    assert_eq!(
        charge.component, "aliased_fixture",
        "a derive'd type is tagged with the crate name, not a module segment"
    );
    let charge_error = &charge.named_type;

    let NamedType::TaggedUnion { variants, .. } = charge_error else {
        panic!("ChargeError must be a tagged_union, got {charge_error:?}");
    };
    let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        names,
        ["Declined", "Overflow"],
        "variants preserved in order"
    );

    let declined = &variants[0];
    let payload = declined
        .payload
        .as_ref()
        .expect("Declined carries a record payload");
    let TypeRef::Record { fields } = payload else {
        panic!("Declined payload must be an inline record, got {payload:?}");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "code");
    assert_eq!(
        fields[0].ty,
        TypeRef::Primitive {
            name: Primitive::I64
        },
        "the recovered field type sees through nothing — it is a plain `i64`"
    );

    // The unit variant carries no payload.
    assert!(
        variants[1].payload.is_none(),
        "Overflow is a unit variant with no payload"
    );
}
