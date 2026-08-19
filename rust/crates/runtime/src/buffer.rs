//! The thread-local event buffer and the emit/drain API annotated code uses.
//!
//! Every annotated operation pushes [`WatchEvent`]s onto a per-thread
//! [`BUFFER`]; the extraction driver drains them with [`take_events`] after
//! running the operation. The buffer holds in-memory events only — there is no
//! serialization or record sink here. [`reset`] clears the buffer between
//! operations.
//!
//! # Threading contract
//!
//! The buffer is thread-local, so a capture is per-thread: an operation and all
//! the events it emits must run on the same thread. Under `cargo test` each test
//! runs on its own thread, so tests' emissions are naturally isolated per test
//! and can be keyed deterministically by the test's name
//! (`std::thread::current().name()` returns the test path under libtest).
//!
//! Escape hatch: if code under test spawns its own threads or async tasks that
//! emit without propagating the buffer, those events land in a different
//! thread-local buffer and won't appear in that test's [`take_events`].
//! Capturing cross-thread/async emission deterministically is future work
//! tracked in the capture-model design (issue #9).

use std::cell::RefCell;

use crate::{Value, WatchEvent};

thread_local! {
    static BUFFER: RefCell<Vec<WatchEvent>> = const { RefCell::new(Vec::new()) };
}

/// Push an `Event { name, value }` onto the thread-local event buffer, encoding
/// `value` as a [`Value::String`].
///
/// The `&str`-taking shim is preserved so macro expansions and call sites that
/// pass `format!("{}", x)` keep compiling unchanged; use [`emit_event_v`] to
/// push an already-structured value.
pub fn emit_event(name: &str, value: &str) {
    emit_event_v(name, Value::String(value.to_string()));
}

/// Push a structured `Event { name, value }` onto the thread-local event buffer.
pub fn emit_event_v(name: &str, value: Value) {
    let event = WatchEvent::Event {
        name: name.to_string(),
        value,
    };
    BUFFER.with(|b| {
        b.borrow_mut().push(event);
    });
}

/// Push a `Run { operation }` marker onto the thread-local event buffer.
pub fn emit_run(operation: &str) {
    let event = WatchEvent::Run {
        operation: operation.to_string(),
    };
    BUFFER.with(|b| {
        b.borrow_mut().push(event);
    });
}

/// Drain and return all buffered events for the current thread, leaving the
/// buffer empty.
#[must_use]
pub fn take_events() -> Vec<WatchEvent> {
    BUFFER.with(|b| std::mem::take(&mut *b.borrow_mut()))
}

/// Clear the thread-local event buffer. Called between operations so captures
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

        let events = take_events();
        assert_eq!(
            events,
            vec![
                WatchEvent::Run {
                    operation: "add".into()
                },
                WatchEvent::Event {
                    name: "add.a".into(),
                    value: Value::String("2".into()),
                },
                WatchEvent::Event {
                    name: "$result".into(),
                    value: Value::Integer(5),
                },
            ]
        );

        // Draining leaves the buffer empty.
        assert!(take_events().is_empty());
    }

    #[test]
    fn reset_clears_buffer() {
        emit_event("x", "1");
        reset();
        assert!(take_events().is_empty());
    }
}
