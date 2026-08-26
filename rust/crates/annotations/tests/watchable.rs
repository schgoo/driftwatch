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
    assert_eq!(
        Status::Active.to_value(),
        map(&[("Active", Value::Map(std::collections::BTreeMap::new()))])
    );
    assert_eq!(
        Status::Named {
            label: "x".to_string()
        }
        .to_value(),
        map(&[("Named", map(&[("label", Value::String("x".into()))]))])
    );
    assert_eq!(
        Status::Wrapped(3).to_value(),
        map(&[("Wrapped", Value::Map(std::collections::BTreeMap::new()))])
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
    let json = annotations::__rt::discovery_json();
    // Struct: only tagged fields, honoring the rename.
    assert!(json.contains("\"name\":\"User\""));
    assert!(json.contains("\"kind\":\"struct\""));
    assert!(json.contains("[\"id\",\"i64\"]"));
    assert!(json.contains("[\"display\",\"String\"]"));
    assert!(!json.contains("secret"));
    // Enum: one variant entry each.
    assert!(json.contains("\"name\":\"Status\""));
    assert!(json.contains("\"kind\":\"enum\""));
    assert!(json.contains("\"name\":\"Named\""));
    assert!(json.contains("[\"label\",\"String\"]"));
    assert!(json.contains("\"name\":\"Active\""));
    assert!(json.contains("\"name\":\"Wrapped\""));
    // Component is the annotated crate's package name (`CARGO_PKG_NAME`), which
    // for this integration-test crate is `annotations`.
    assert!(json.contains("\"component\":\"annotations\""));
}
