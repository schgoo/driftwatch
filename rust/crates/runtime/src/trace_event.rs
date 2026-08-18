//! The [`TraceEvent`] record type.

use serde::{Deserialize, Serialize};

use crate::Value;

/// One emitted trace record: a named [`TraceEvent::Event`] carrying a value, or
/// a [`TraceEvent::Run`] marking the start of an operation.
///
/// The serde form tags each record with a `kind` field (`"Event"` or `"Run"`).
/// That tag is the stable on-disk discriminator read by capture parsers, so
/// renaming it is a breaking change to the trace artifact format.
///
/// # Examples
///
/// ```
/// use driftwatch_runtime::{TraceEvent, Value};
///
/// let event = TraceEvent::Event { name: "count".into(), value: Value::Integer(1) };
/// assert_eq!(event.name(), "count");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[expect(
    clippy::exhaustive_enums,
    reason = "the wire form of a trace record; adding a variant is a deliberate format change that must be handled everywhere"
)]
pub enum TraceEvent {
    /// A named event carrying a structured value.
    Event {
        /// The event name (for example an operation input or `$result`).
        name: String,
        /// The captured value.
        value: Value,
    },
    /// Marks the start of an operation by name.
    Run {
        /// The operation name.
        operation: String,
    },
}

impl TraceEvent {
    /// Returns the event name, or the operation name for a `Run`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            TraceEvent::Event { name, .. } => name,
            TraceEvent::Run { operation } => operation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_event_jsonl_roundtrips() {
        // Record mode persists events as JSONL; each line must round-trip back
        // into the same TraceEvent (the capture driver parses these).
        let events = vec![
            TraceEvent::Run {
                operation: "add".into(),
            },
            TraceEvent::Event {
                name: "add.a".into(),
                value: Value::String("2".into()),
            },
            TraceEvent::Event {
                name: "$result".into(),
                value: Value::String("5".into()),
            },
        ];
        for ev in &events {
            let line = serde_json::to_string(ev).unwrap();
            let back: TraceEvent = serde_json::from_str(&line).unwrap();
            assert_eq!(&back, ev);
        }
        // The tag discriminates the two shapes.
        let run_line = serde_json::to_string(&events[0]).unwrap();
        assert!(run_line.contains("\"kind\":\"Run\""), "{run_line}");
    }
}
