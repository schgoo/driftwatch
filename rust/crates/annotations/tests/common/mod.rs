//! Shared helpers for the annotation integration tests.
//!
//! The builders construct the span-shaped pieces the emission assertions compare
//! against: [`op_attrs`] builds a `conformance.operation` span's three
//! attributes, and [`obs`]/[`result`]/[`empty`]/[`error`] build the completion
//! and observation [`SpanEvent`]s. Tests compare against `take_spans()` and read
//! `.name`/`.attributes`/`.events`; ids and ticks are never asserted.
//!
//! Not every test file uses every helper, so this module allows dead code.
#![allow(
    dead_code,
    reason = "shared across test binaries; not every binary uses every helper"
)]

use annotations::{EventName, SpanEvent, ToValue, Value};
use std::collections::BTreeMap;

/// The three `conformance.operation` span attributes: `component.id`,
/// `operation.name`, and the `operation.inputs` kvlist.
pub fn op_attrs(component: &str, name: &str, inputs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    let kv: BTreeMap<String, Value> = inputs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect();
    BTreeMap::from([
        (
            "conformance.component.id".to_string(),
            Value::String(component.to_string()),
        ),
        (
            "conformance.operation.name".to_string(),
            Value::String(name.to_string()),
        ),
        ("conformance.operation.inputs".to_string(), Value::Map(kv)),
    ])
}

/// A `conformance.observation` event (`observation.name` + `observation.value`).
pub fn obs(name: &str, value: impl ToValue) -> SpanEvent {
    SpanEvent {
        name: EventName::Observation,
        attributes: BTreeMap::from([
            (
                "conformance.observation.name".to_string(),
                Value::String(name.to_string()),
            ),
            (
                "conformance.observation.value".to_string(),
                value.to_value(),
            ),
        ]),
    }
}

/// A `conformance.result` completion event (`result.value`).
pub fn result(value: impl ToValue) -> SpanEvent {
    SpanEvent {
        name: EventName::Result,
        attributes: BTreeMap::from([("conformance.result.value".to_string(), value.to_value())]),
    }
}

/// A `conformance.empty` completion event (no value attribute).
pub fn empty() -> SpanEvent {
    SpanEvent {
        name: EventName::Empty,
        attributes: BTreeMap::new(),
    }
}

/// A `conformance.error` completion event (`error.name` + `error.value`).
pub fn error(name: &str, value: impl ToValue) -> SpanEvent {
    SpanEvent {
        name: EventName::Error,
        attributes: BTreeMap::from([
            (
                "conformance.error.name".to_string(),
                Value::String(name.to_string()),
            ),
            ("conformance.error.value".to_string(), value.to_value()),
        ]),
    }
}
