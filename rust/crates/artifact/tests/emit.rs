//! Integration tests for the CTSC OTLP artifact emitter.
//!
//! These build hand-constructed captures (bypassing the thread-local runtime
//! buffer for determinism) and assert the exact `serde_json::Value` shape of the
//! emitted OTLP `TracesData`.

use std::collections::{BTreeMap, BTreeSet};

use artifact::{OtlpFormat, Resource, TraceCapture};
use runtime::{EventName, Span, SpanEvent, SpanName, SpanStatus, Value};
use serde_json::Value as Json;

/// A trace id shared by a capture; low bytes hold a nonzero counter.
fn trace_id(n: u8) -> [u8; 16] {
    let mut id = [0_u8; 16];
    id[15] = n;
    id
}

fn span_id(n: u8) -> [u8; 8] {
    let mut id = [0_u8; 8];
    id[7] = n;
    id
}

fn attrs(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn event(name: EventName, time: u64, pairs: &[(&str, Value)]) -> SpanEvent {
    SpanEvent {
        name,
        time,
        attributes: attrs(pairs),
    }
}

/// A resource whose five required attributes are all present.
fn resource() -> Resource {
    Resource::new("driftwatch", "0.1.0", "order-pricing", "rust")
}

/// A nested `run -> scenario -> operation -> dep` capture.
fn nested_capture() -> TraceCapture {
    let tid = trace_id(1);
    let run = Span {
        trace_id: tid,
        span_id: span_id(1),
        parent_span_id: None,
        name: SpanName::Run,
        start: 1,
        end: Some(12),
        status: SpanStatus::Unset,
        attributes: attrs(&[("conformance.run.id", Value::String("run-1".to_string()))]),
        events: Vec::new(),
    };
    let scenario = Span {
        trace_id: tid,
        span_id: span_id(2),
        parent_span_id: Some(span_id(1)),
        name: SpanName::Scenario,
        start: 2,
        end: Some(11),
        status: SpanStatus::Unset,
        attributes: attrs(&[
            (
                "conformance.scenario.name",
                Value::String("gold".to_string()),
            ),
            ("conformance.scenario.index", Value::Integer(0)),
        ]),
        events: Vec::new(),
    };
    let operation = Span {
        trace_id: tid,
        span_id: span_id(3),
        parent_span_id: Some(span_id(2)),
        name: SpanName::Operation,
        start: 3,
        end: Some(10),
        status: SpanStatus::Unset,
        attributes: attrs(&[
            (
                "conformance.component.id",
                Value::String("example.pricing".to_string()),
            ),
            (
                "conformance.operation.name",
                Value::String("price_order".to_string()),
            ),
            (
                "conformance.operation.inputs",
                Value::Map(attrs(&[("qty", Value::Integer(10))])),
            ),
        ]),
        events: vec![
            event(
                EventName::Observation,
                4,
                &[
                    (
                        "conformance.observation.name",
                        Value::String("subtotal".to_string()),
                    ),
                    ("conformance.observation.value", Value::Integer(15000)),
                ],
            ),
            event(
                EventName::Result,
                6,
                &[("conformance.result.value", Value::Integer(13068))],
            ),
        ],
    };
    let dep = Span {
        trace_id: tid,
        span_id: span_id(4),
        parent_span_id: Some(span_id(3)),
        name: SpanName::Operation,
        start: 7,
        end: Some(9),
        status: SpanStatus::Unset,
        attributes: attrs(&[
            (
                "conformance.component.id",
                Value::String("example.pricing".to_string()),
            ),
            (
                "conformance.operation.name",
                Value::String("tax_rate".to_string()),
            ),
            ("conformance.operation.inputs", Value::Map(BTreeMap::new())),
        ]),
        events: vec![event(EventName::Empty, 8, &[])],
    };
    TraceCapture {
        resource: resource(),
        spans: vec![run, scenario, operation, dep],
    }
}

fn parse(json: &str) -> Json {
    serde_json::from_str(json).expect("emitted JSON must parse")
}

fn spans_of(traces: &Json) -> &Vec<Json> {
    traces["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .expect("spans array")
}

/// Looks up a `KeyValue`'s `AnyValue` by key in an OTLP attributes array.
fn any_value<'a>(attributes: &'a Json, key: &str) -> &'a Json {
    attributes
        .as_array()
        .expect("attributes array")
        .iter()
        .find(|kv| kv["key"] == Json::String(key.to_string()))
        .map_or_else(|| panic!("missing attribute {key}"), |kv| &kv["value"])
}

#[test]
fn required_resource_attributes_present() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    let resource_attrs = &traces["resourceSpans"][0]["resource"]["attributes"];
    assert_eq!(
        any_value(resource_attrs, "conformance.version"),
        &serde_json::json!({ "stringValue": "0.1.0" })
    );
    for key in [
        "conformance.tool.name",
        "conformance.tool.version",
        "conformance.target.name",
        "conformance.target.language",
    ] {
        assert!(
            any_value(resource_attrs, key)["stringValue"].is_string(),
            "{key} must be a stringValue"
        );
    }
    // Linked-only attributes are omitted at Trace Core.
    assert!(
        resource_attrs
            .as_array()
            .expect("array")
            .iter()
            .all(|kv| kv["key"] != Json::String("service.name".to_string())),
        "service.name must not be emitted"
    );
}

#[test]
fn scope_and_schema_present() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    let scope_spans = &traces["resourceSpans"][0]["scopeSpans"][0];
    assert!(scope_spans["scope"]["name"].is_string());
    assert!(scope_spans["scope"]["version"].is_string());
    assert!(scope_spans["schemaUrl"].is_string());
    assert!(traces["resourceSpans"][0]["schemaUrl"].is_string());
}

#[test]
fn span_ids_are_lowercase_hex_of_the_right_length() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    for span in spans_of(&traces) {
        let trace = span["traceId"].as_str().expect("traceId string");
        assert_eq!(trace.len(), 32);
        assert!(
            trace
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(trace.trim_matches('0'), "", "traceId must be nonzero");
        let sid = span["spanId"].as_str().expect("spanId string");
        assert_eq!(sid.len(), 16);
        assert!(
            sid.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}

#[test]
fn root_omits_parent_and_children_link() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    let spans = spans_of(&traces);
    // Run is the root.
    assert!(
        spans[0].get("parentSpanId").is_none(),
        "root omits parentSpanId"
    );
    // Scenario/operation/dep link to their parent.
    assert_eq!(
        spans[1]["parentSpanId"], spans[0]["spanId"],
        "scenario parents run"
    );
    assert_eq!(
        spans[2]["parentSpanId"], spans[1]["spanId"],
        "operation parents scenario"
    );
    assert_eq!(
        spans[3]["parentSpanId"], spans[2]["spanId"],
        "dep parents operation"
    );
}

#[test]
fn span_scalar_fields() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    let spans = spans_of(&traces);
    let op = &spans[2];
    assert_eq!(
        op["name"],
        Json::String("conformance.operation".to_string())
    );
    assert_eq!(op["kind"], serde_json::json!(1));
    // Timestamps are decimal strings.
    assert_eq!(op["startTimeUnixNano"], Json::String("3".to_string()));
    assert_eq!(op["endTimeUnixNano"], Json::String("10".to_string()));
    assert_eq!(op["status"], serde_json::json!({ "code": 1 }));
    // Event timeUnixNano is also a decimal string.
    let observation = &op["events"][0];
    assert_eq!(observation["timeUnixNano"], Json::String("4".to_string()));
    assert_eq!(
        observation["name"],
        Json::String("conformance.observation".to_string())
    );
}

#[test]
fn unset_status_promotes_to_ok_error_maps_to_two() {
    let mut capture = nested_capture();
    // Flip the operation to Error.
    capture.spans[2].status = SpanStatus::Error;
    let traces = parse(&capture.to_otlp(OtlpFormat::Json));
    let spans = spans_of(&traces);
    assert_eq!(spans[0]["status"]["code"], serde_json::json!(1)); // Unset -> 1
    assert_eq!(spans[2]["status"]["code"], serde_json::json!(2)); // Error -> 2
}

#[test]
fn empty_event_has_no_result_value() {
    let traces = parse(&nested_capture().to_otlp(OtlpFormat::Json));
    let spans = spans_of(&traces);
    let dep_event = &spans[3]["events"][0];
    assert_eq!(
        dep_event["name"],
        Json::String("conformance.empty".to_string())
    );
    assert_eq!(dep_event["attributes"], serde_json::json!([]));
}

/// Emits a single-value observation and returns its `AnyValue` JSON.
fn any_value_of(value: Value) -> Json {
    let span = Span {
        trace_id: trace_id(2),
        span_id: span_id(1),
        parent_span_id: None,
        name: SpanName::Operation,
        start: 1,
        end: Some(3),
        status: SpanStatus::Unset,
        attributes: BTreeMap::new(),
        events: vec![event(EventName::Observation, 2, &[("v", value)])],
    };
    let capture = TraceCapture {
        resource: resource(),
        spans: vec![span],
    };
    let traces = parse(&capture.to_otlp(OtlpFormat::Json));
    let attributes = &spans_of(&traces)[0]["events"][0]["attributes"];
    any_value(attributes, "v").clone()
}

#[test]
fn anyvalue_scalars() {
    assert_eq!(
        any_value_of(Value::String("hi".to_string())),
        serde_json::json!({ "stringValue": "hi" })
    );
    assert_eq!(
        any_value_of(Value::Integer(-42)),
        serde_json::json!({ "intValue": "-42" })
    );
    assert_eq!(
        any_value_of(Value::Integer(i64::MAX)),
        serde_json::json!({ "intValue": "9223372036854775807" })
    );
    assert_eq!(
        any_value_of(Value::Bool(true)),
        serde_json::json!({ "boolValue": true })
    );
}

#[test]
fn anyvalue_floats_finite_and_non_finite() {
    assert_eq!(
        any_value_of(Value::Float(1.5)),
        serde_json::json!({ "doubleValue": 1.5 })
    );
    // -0.0 is finite and stays a number.
    assert_eq!(
        any_value_of(Value::Float(-0.0)),
        serde_json::json!({ "doubleValue": -0.0 })
    );
    assert_eq!(
        any_value_of(Value::Float(f64::NAN)),
        serde_json::json!({ "doubleValue": "NaN" })
    );
    // A distinct NaN bit-pattern normalizes to the same "NaN".
    let weird_nan = f64::from_bits(0x7ff8_0000_0000_0001);
    assert!(weird_nan.is_nan());
    assert_eq!(
        any_value_of(Value::Float(weird_nan)),
        serde_json::json!({ "doubleValue": "NaN" })
    );
    assert_eq!(
        any_value_of(Value::Float(f64::INFINITY)),
        serde_json::json!({ "doubleValue": "Infinity" })
    );
    assert_eq!(
        any_value_of(Value::Float(f64::NEG_INFINITY)),
        serde_json::json!({ "doubleValue": "-Infinity" })
    );
}

#[test]
fn anyvalue_list_and_set_and_empty_collections() {
    assert_eq!(
        any_value_of(Value::List(vec![Value::Integer(1), Value::Integer(2)])),
        serde_json::json!({
            "arrayValue": { "values": [ { "intValue": "1" }, { "intValue": "2" } ] }
        })
    );
    // A set wires as an array too, in sorted (BTreeSet) order.
    assert_eq!(
        any_value_of(Value::Set(BTreeSet::from([
            Value::Integer(2),
            Value::Integer(1)
        ]))),
        serde_json::json!({
            "arrayValue": { "values": [ { "intValue": "1" }, { "intValue": "2" } ] }
        })
    );
    assert_eq!(
        any_value_of(Value::List(Vec::new())),
        serde_json::json!({ "arrayValue": { "values": [] } })
    );
    assert_eq!(
        any_value_of(Value::Map(BTreeMap::new())),
        serde_json::json!({ "kvlistValue": { "values": [] } })
    );
}

#[test]
fn anyvalue_map_and_variant() {
    assert_eq!(
        any_value_of(Value::Map(attrs(&[
            ("a", Value::Integer(1)),
            ("b", Value::Bool(false)),
        ]))),
        serde_json::json!({
            "kvlistValue": { "values": [
                { "key": "a", "value": { "intValue": "1" } },
                { "key": "b", "value": { "boolValue": false } },
            ] }
        })
    );
    // Variant with a payload.
    assert_eq!(
        any_value_of(Value::variant("Some", Value::String("x".to_string()))),
        serde_json::json!({
            "kvlistValue": { "values": [
                { "key": "Some", "value": { "stringValue": "x" } },
            ] }
        })
    );
    // Payload-less variant is a single-key kvlist whose payload is empty unit.
    assert_eq!(
        any_value_of(Value::variant_unit("Active")),
        serde_json::json!({
            "kvlistValue": { "values": [
                { "key": "Active", "value": { "kvlistValue": { "values": [] } } },
            ] }
        })
    );
}

#[test]
fn jsonl_is_newline_terminated_and_each_line_is_valid_traces_data() {
    let jsonl = nested_capture().to_otlp(OtlpFormat::Jsonl);
    assert!(jsonl.ends_with('\n'), "jsonl must be newline-terminated");
    let lines: Vec<&str> = jsonl.lines().collect();
    assert_eq!(lines.len(), 1, "one capture renders one line");
    for line in lines {
        let value = parse(line);
        assert!(
            value["resourceSpans"].is_array(),
            "each line must be a TracesData"
        );
    }
}

#[test]
fn fault_event_carries_type_observer_and_message() {
    let span = Span {
        trace_id: trace_id(3),
        span_id: span_id(1),
        parent_span_id: None,
        name: SpanName::Operation,
        start: 1,
        end: Some(3),
        status: SpanStatus::Error,
        attributes: attrs(&[
            ("conformance.component.id", Value::String("c".to_string())),
            (
                "conformance.operation.name",
                Value::String("op".to_string()),
            ),
            ("conformance.operation.inputs", Value::Map(BTreeMap::new())),
        ]),
        events: vec![event(
            EventName::Fault,
            2,
            &[
                (
                    "conformance.fault.type",
                    Value::String("unexpected".to_string()),
                ),
                (
                    "conformance.fault.observer",
                    Value::String("target".to_string()),
                ),
                (
                    "conformance.fault.message",
                    Value::String("boom".to_string()),
                ),
            ],
        )],
    };
    let capture = TraceCapture {
        resource: resource(),
        spans: vec![span],
    };
    let traces = parse(&capture.to_otlp(OtlpFormat::Json));
    let fault = &spans_of(&traces)[0]["events"][0];
    let fault_attrs = &fault["attributes"];
    assert_eq!(
        any_value(fault_attrs, "conformance.fault.type"),
        &serde_json::json!({ "stringValue": "unexpected" })
    );
    assert_eq!(
        any_value(fault_attrs, "conformance.fault.observer"),
        &serde_json::json!({ "stringValue": "target" })
    );
    assert_eq!(spans_of(&traces)[0]["status"]["code"], serde_json::json!(2));
}
