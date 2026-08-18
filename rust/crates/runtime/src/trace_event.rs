//! The [`TraceEvent`] record type.

use crate::Value;

/// One emitted trace record: a named [`TraceEvent::Event`] carrying a value, or
/// a [`TraceEvent::Run`] marking the start of an operation.
///
/// # Examples
///
/// ```
/// use runtime::{TraceEvent, Value};
///
/// let event = TraceEvent::Event { name: "count".into(), value: Value::Integer(1) };
/// assert_eq!(event.name(), "count");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
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
    fn names() {
        let event = TraceEvent::Event {
            name: "count".into(),
            value: Value::Integer(1),
        };
        assert_eq!(event.name(), "count");

        let run = TraceEvent::Run {
            operation: "add".into(),
        };
        assert_eq!(run.name(), "add");
    }
}
