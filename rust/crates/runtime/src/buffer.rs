//! The thread-local trace buffer and the emit/drain API annotated code uses.
//!
//! Every annotated operation pushes [`TraceEvent`]s onto a per-thread
//! [`BUFFER`]; the extraction driver drains them with [`take_traces`] after
//! running the operation. The buffer holds in-memory events only — there is no
//! serialization or record sink here. [`reset`] clears the buffer between
//! operations.

use std::cell::RefCell;

use crate::{TraceEvent, Value};

thread_local! {
    static BUFFER: RefCell<Vec<TraceEvent>> = const { RefCell::new(Vec::new()) };
}

/// Push an `Event { name, value }` onto the thread-local trace buffer, encoding
/// `value` as a [`Value::String`].
///
/// The `&str`-taking shim is preserved so macro expansions and call sites that
/// pass `format!("{}", x)` keep compiling unchanged; use [`emit_event_v`] to
/// push an already-structured value.
pub fn emit_event(name: &str, value: &str) {
    emit_event_v(name, Value::String(value.to_string()));
}

/// Push a structured `Event { name, value }` onto the thread-local trace buffer.
pub fn emit_event_v(name: &str, value: Value) {
    let event = TraceEvent::Event {
        name: name.to_string(),
        value,
    };
    BUFFER.with(|b| {
        b.borrow_mut().push(event);
    });
}

/// Push a `Run { operation }` marker onto the thread-local trace buffer.
pub fn emit_run(operation: &str) {
    let event = TraceEvent::Run {
        operation: operation.to_string(),
    };
    BUFFER.with(|b| {
        b.borrow_mut().push(event);
    });
}

/// Drain and return all buffered events for the current thread, leaving the
/// buffer empty.
#[must_use]
pub fn take_traces() -> Vec<TraceEvent> {
    BUFFER.with(|b| std::mem::take(&mut *b.borrow_mut()))
}

/// Clear the thread-local trace buffer. Called between operations so captures
/// do not leak into one another.
pub fn reset() {
    BUFFER.with(|b| b.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_take_and_reset() {
        reset();
        emit_run("add");
        emit_event("add.a", "2");
        emit_event_v("$result", Value::Integer(5));

        let traces = take_traces();
        assert_eq!(
            traces,
            vec![
                TraceEvent::Run {
                    operation: "add".into()
                },
                TraceEvent::Event {
                    name: "add.a".into(),
                    value: Value::String("2".into()),
                },
                TraceEvent::Event {
                    name: "$result".into(),
                    value: Value::Integer(5),
                },
            ]
        );

        // Draining leaves the buffer empty.
        assert!(take_traces().is_empty());
    }

    #[test]
    fn reset_clears_buffer() {
        emit_event("x", "1");
        reset();
        assert!(take_traces().is_empty());
    }
}
