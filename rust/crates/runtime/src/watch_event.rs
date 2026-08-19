//! The [`WatchEvent`] record type.

use crate::Value;

/// One emitted event record: a named [`WatchEvent::Event`] carrying a value, or
/// a [`WatchEvent::Run`] marking the start of an operation.
///
/// # Examples
///
/// ```
/// use runtime::{Value, WatchEvent};
///
/// let event = WatchEvent::Event { name: "count".into(), value: Value::Integer(1) };
/// assert_eq!(event.name(), "count");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "the wire form of an event record; adding a variant is a deliberate format change that must be handled everywhere"
)]
pub enum WatchEvent {
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

impl WatchEvent {
    /// Returns the event name, or the operation name for a `Run`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            WatchEvent::Event { name, .. } => name,
            WatchEvent::Run { operation } => operation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let event = WatchEvent::Event {
            name: "count".into(),
            value: Value::Integer(1),
        };
        assert_eq!(event.name(), "count");

        let run = WatchEvent::Run {
            operation: "add".into(),
        };
        assert_eq!(run.name(), "add");
    }
}
