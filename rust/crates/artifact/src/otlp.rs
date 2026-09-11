//! The span-tree → OTLP `TracesData` `serde_json::Value` builder (CTSC §§2–7).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use runtime::{Span, SpanEvent, SpanStatus, Value};
use serde_json::{Map as JsonMap, Value as Json, json};

use crate::anyvalue::key_value;
use crate::resource::Resource;

/// The stable Driftwatch instrumentation-scope name (OTLP `scope.name`).
const SCOPE_NAME: &str = "driftwatch.runtime";

/// The CTSC schema URL emitted at `ScopeSpans` and `ResourceSpans` level.
const SCHEMA_URL: &str = "https://driftwatch.dev/ctsc/schema/0.1.0";

/// Builds the full OTLP `TracesData` object for one capture.
pub(crate) fn traces_data(resource: &Resource, spans: &[Span]) -> Json {
    let scope_spans = json!({
        "scope": {
            "name": SCOPE_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        },
        "spans": spans.iter().map(span_json).collect::<Vec<_>>(),
        "schemaUrl": SCHEMA_URL,
    });
    json!({
        "resourceSpans": [
            {
                "resource": { "attributes": resource_attributes(resource) },
                "scopeSpans": [scope_spans],
                "schemaUrl": SCHEMA_URL,
            }
        ]
    })
}

fn resource_attributes(resource: &Resource) -> Vec<Json> {
    resource
        .attributes()
        .iter()
        .map(|(key, value)| key_value(key, value))
        .collect()
}

fn attributes_json(attributes: &BTreeMap<String, Value>) -> Vec<Json> {
    attributes
        .iter()
        .map(|(key, value)| key_value(key, value))
        .collect()
}

fn span_json(span: &Span) -> Json {
    let mut obj = JsonMap::new();
    obj.insert("traceId".to_string(), json!(hex(&span.trace_id)));
    obj.insert("spanId".to_string(), json!(hex(&span.span_id)));
    // Root spans omit `parentSpanId` entirely (CTSC / OTLP).
    if let Some(parent) = span.parent_span_id {
        obj.insert("parentSpanId".to_string(), json!(hex(&parent)));
    }
    obj.insert("name".to_string(), json!(span.name.as_ctsc_str()));
    obj.insert("kind".to_string(), json!(1));
    obj.insert(
        "startTimeUnixNano".to_string(),
        json!(span.start.to_string()),
    );
    // A closed span always has an `end`; fall back to `start` defensively.
    let end = span.end.unwrap_or(span.start);
    obj.insert("endTimeUnixNano".to_string(), json!(end.to_string()));
    obj.insert(
        "attributes".to_string(),
        json!(attributes_json(&span.attributes)),
    );
    if !span.events.is_empty() {
        obj.insert(
            "events".to_string(),
            json!(span.events.iter().map(event_json).collect::<Vec<_>>()),
        );
    }
    obj.insert(
        "status".to_string(),
        json!({ "code": status_code(span.status) }),
    );
    Json::Object(obj)
}

fn event_json(event: &SpanEvent) -> Json {
    json!({
        "timeUnixNano": event.time.to_string(),
        "name": event.name.as_ctsc_str(),
        "attributes": attributes_json(&event.attributes),
    })
}

/// Maps the CTSC span status to its numeric OTLP `StatusCode`. `Unset` promotes
/// to `OK` (`1`) — safe because the runtime sets `Error` on both the
/// declared-error and fault paths, so an unset span is a success.
fn status_code(status: SpanStatus) -> u8 {
    match status {
        SpanStatus::Unset | SpanStatus::Ok => 1,
        SpanStatus::Error => 2,
    }
}

/// Encodes id bytes as lowercase hex (16 → 32 chars for a trace id, 8 → 16 for a
/// span id).
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing to a String is infallible");
    }
    out
}
