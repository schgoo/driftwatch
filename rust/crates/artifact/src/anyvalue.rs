//! [`runtime::Value`] → OTLP `AnyValue` `serde_json::Value` (CTSC §8).

use runtime::Value;
use serde_json::{Value as Json, json};

/// Renders one [`runtime::Value`] as an OTLP `AnyValue` object.
///
/// Follows CTSC §8: integers and non-finite floats serialize as strings, a
/// `Set` wires identically to a `List` (a stable, already-sorted `arrayValue`),
/// a `Map` as a `kvlistValue`, and a `Variant` as a single-key `kvlistValue`
/// tagged union.
pub(crate) fn any_value(value: &Value) -> Json {
    match value {
        Value::String(s) => json!({ "stringValue": s }),
        Value::Integer(i) => json!({ "intValue": i.to_string() }),
        Value::Bool(b) => json!({ "boolValue": b }),
        Value::Float(f) => float_value(*f),
        Value::List(items) => array_value(items.iter()),
        // A `BTreeSet` iterates in sorted order, so this is already deterministic.
        Value::Set(items) => array_value(items.iter()),
        Value::Map(map) => kvlist_value(map.iter().map(|(k, v)| (k.as_str(), v))),
        Value::Variant { tag, value } => kvlist_value(std::iter::once((tag.as_str(), &**value))),
    }
}

/// Builds a `{ "key", "value" }` OTLP `KeyValue`. Shared with the span/event
/// attribute list builder in `otlp`.
pub(crate) fn key_value(key: &str, value: &Value) -> Json {
    json!({ "key": key, "value": any_value(value) })
}

fn array_value<'a>(items: impl Iterator<Item = &'a Value>) -> Json {
    let values: Vec<Json> = items.map(any_value).collect();
    json!({ "arrayValue": { "values": values } })
}

fn kvlist_value<'a>(entries: impl Iterator<Item = (&'a str, &'a Value)>) -> Json {
    let values: Vec<Json> = entries.map(|(k, v)| key_value(k, v)).collect();
    json!({ "kvlistValue": { "values": values } })
}

/// Maps an `f64` to an OTLP `doubleValue`: a JSON number when finite, else the
/// canonical non-finite string. Every NaN bit-pattern normalizes to `"NaN"`
/// (OTLP does not preserve NaN payloads), so bitwise-distinct NaNs never surface
/// as false drift.
fn float_value(f: f64) -> Json {
    if f.is_nan() {
        json!({ "doubleValue": "NaN" })
    } else if f.is_infinite() {
        json!({ "doubleValue": if f > 0.0 { "Infinity" } else { "-Infinity" } })
    } else {
        let number =
            serde_json::Number::from_f64(f).expect("a finite f64 is always a valid JSON number");
        json!({ "doubleValue": number })
    }
}
