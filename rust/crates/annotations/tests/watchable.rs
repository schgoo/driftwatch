//! `#[derive(Watchable)]` for structs and enums: `to_value` shapes,
//! `emit_fields` events, and `TypeMeta`/`VariantMeta` discovery registration.
//!
//! Unlike the other emission tests (which gate each `#[test]` with
//! `#[cfg_attr(not(feature = "trace"), ignore)]` because `#[watch_operation]`
//! expands to identity and the annotated function still exists off-trace),
//! `#[derive(Watchable)]` off-trace emits **no impls at all**. Any body that
//! calls `.to_value()`/`.emit_fields()` or names the `Watchable` trait cannot
//! compile without `trace`, so the whole file is gated: it compiles empty
//! (0 tests) under default features and runs fully under `--all-features`.
#![cfg(feature = "trace")]

use annotations::__rt::Watchable as _;
use annotations::{ToValue, Value, Watchable, reset, take_events};

mod common;
use common::ev;
use serde_json::json;

#[derive(Watchable)]
struct User {
    #[watchable]
    id: i64,
    #[watchable(name = "display")]
    name: String,
    #[allow(
        dead_code,
        reason = "untagged field is intentionally excluded from the watch surface"
    )]
    secret: String,
}

#[derive(Watchable)]
enum Status {
    Active,
    Named {
        label: String,
    },
    #[allow(
        dead_code,
        reason = "tuple payload exercises the tuple-variant arm; its value is not read"
    )]
    Wrapped(i64),
}

fn map(pairs: &[(&str, Value)]) -> Value {
    Value::Map(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    )
}

#[test]
fn struct_to_value_uses_only_tagged_fields_with_renames() {
    let u = User {
        id: 7,
        name: "ada".to_string(),
        secret: "shh".to_string(),
    };
    assert_eq!(
        u.to_value(),
        map(&[
            ("id", Value::Integer(7)),
            ("display", Value::String("ada".into()))
        ])
    );
}

#[test]
fn struct_emit_fields_emits_tagged_fields() {
    reset();
    let u = User {
        id: 7,
        name: "ada".to_string(),
        secret: "shh".to_string(),
    };
    u.emit_fields(None);
    assert_eq!(take_events(), vec![ev("id", 7), ev("display", "ada")]);
}

#[test]
fn enum_to_value_tags_each_variant_shape() {
    // Each variant projects to a `Value::Variant` (tagged union, CTSC 8.5): a
    // payload-less arm carries CTSC unit (an empty Map); a named arm carries a
    // string-keyed map of its fields.
    assert_eq!(Status::Active.to_value(), Value::variant_unit("Active"));
    assert_eq!(
        Status::Named {
            label: "x".to_string()
        }
        .to_value(),
        Value::variant("Named", map(&[("label", Value::String("x".into()))]))
    );
    assert_eq!(
        Status::Wrapped(3).to_value(),
        Value::variant_unit("Wrapped")
    );
}

#[test]
fn enum_emit_fields_named_variant_emits_base_and_fields() {
    reset();
    Status::Named {
        label: "x".to_string(),
    }
    .emit_fields(None);
    assert_eq!(
        take_events(),
        vec![ev("status", "Named"), ev("status.label", "x")]
    );
}

#[test]
fn discovery_registers_struct_and_enum_types() {
    // Parse (not substring-match) to prove the envelope is well-formed JSON and
    // to navigate it structurally. The type *order* is link-order and not
    // guaranteed, so find each type by name; each type's fields/variants are in
    // source order (deterministic) and asserted exactly.
    let json = annotations::__rt::discovery_json();
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("discovery_json must be valid JSON");
    let types = parsed["types"]
        .as_array()
        .expect("`types` must be an array");
    let find = |n: &str| {
        types
            .iter()
            .find(|t| t["name"] == n)
            .unwrap_or_else(|| panic!("discovery is missing type `{n}`"))
    };

    // Struct: only the tagged fields, honoring the rename; `secret` is excluded.
    let user = find("User");
    assert_eq!(user["kind"], "struct");
    assert_eq!(user["component"], "annotations");
    assert_eq!(
        user["fields"],
        json!([["id", "i64"], ["display", "String"]])
    );
    assert_eq!(user["variants"], json!([]));

    // Enum: no top-level fields, one entry per variant in declaration order,
    // named-variant fields carried through.
    let status = find("Status");
    assert_eq!(status["kind"], "enum");
    assert_eq!(status["component"], "annotations");
    assert_eq!(status["fields"], json!([]));
    assert_eq!(
        status["variants"],
        json!([
            { "name": "Active", "fields": [] },
            { "name": "Named", "fields": [["label", "String"]] },
            { "name": "Wrapped", "fields": [] },
        ])
    );
}
