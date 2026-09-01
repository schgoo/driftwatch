//! The OTLP-shaped span model and the thread-local span buffer/context.
//!
//! CTSC (see `docs/trace-contract.md`) models a capture as a tree of spans
//! (`conformance.run` / `scenario` / `operation`) each carrying
//! ordered [`SpanEvent`]s. This module is the runtime substrate for that tree:
//! the [`Span`] record, a per-thread buffer of opened spans, and a current-span
//! stack so nested [`open_span`] calls link to their parent.
//!
//! This module builds the in-memory span tree only; serializing it to OTLP is
//! the responsibility of a separate layer. It is independent of the flat
//! [`crate::emit_event`] / [`crate::WatchEvent`] event path.
//!
//! # Ids and timestamps
//!
//! `trace_id`, `span_id`, `start`, and `end` are minted from deterministic
//! per-thread counters so goldens stay stable. Per CTSC Strict (comparison
//! §7.9) these values are **never compared** across runs: `trace_id`/`span_id`
//! establish nesting only, and `start`/`end` ticks establish sibling order and
//! parent/child containment only. Two separate monotonic counters drive them —
//! a span-id counter and a clock/tick counter that advances on every span open
//! *and* close, so nested spans get strictly non-overlapping, properly ordered
//! intervals (`parent.start < child.start < child.end < parent.end`).
//!
//! # Threading contract
//!
//! Like [`crate::take_events`], the buffer and stack are thread-local, and
//! #37a is single-threaded: spans are opened and closed on one thread. Linking
//! spans opened on a spawned thread back to a parent (cross-thread propagation
//! for parallel branches) is tracked in #42.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use crate::Value;

/// The closed set of CTSC span names. Maps to the fixed `conformance.*` span
/// names (CTSC trace §6); the domain name is an attribute, not the span name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "the closed CTSC span vocabulary; adding a name is a deliberate format change that must be handled everywhere"
)]
pub enum SpanName {
    /// One target invocation, e.g. a `cargo test` run (`conformance.run`).
    Run,
    /// One observed test/scenario (`conformance.scenario`).
    Scenario,
    /// A `watch_operation`/`watch_dep` invocation (`conformance.operation`).
    Operation,
    // TODO(#42): add `Parallel` (`conformance.parallel`) for cross-thread branch emission.
}

impl SpanName {
    /// Returns the CTSC span name string for later serialization.
    #[must_use]
    pub fn as_ctsc_str(self) -> &'static str {
        match self {
            SpanName::Run => "conformance.run",
            SpanName::Scenario => "conformance.scenario",
            SpanName::Operation => "conformance.operation",
        }
    }
}

/// The closed set of CTSC event names (CTSC trace §7). Maps to the fixed
/// `conformance.*` event names that may repeat within an operation span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "the closed CTSC event vocabulary; adding a name is a deliberate format change that must be handled everywhere"
)]
pub enum EventName {
    /// A body echo / `watch_point` / field-mutation echo (`conformance.observation`).
    Observation,
    /// A success value (`conformance.result`).
    Result,
    /// Deliberate absence, e.g. Rust `None` (`conformance.empty`).
    Empty,
    /// A declared error, e.g. Rust `Err` (`conformance.error`).
    Error,
    /// An unexpected failure, e.g. Rust `panic!` (`conformance.fault`).
    Fault,
}

impl EventName {
    /// Returns the CTSC event name string for later serialization.
    #[must_use]
    pub fn as_ctsc_str(self) -> &'static str {
        match self {
            EventName::Observation => "conformance.observation",
            EventName::Result => "conformance.result",
            EventName::Empty => "conformance.empty",
            EventName::Error => "conformance.error",
            EventName::Fault => "conformance.fault",
        }
    }
}

/// One event recorded on a span, in emission order (CTSC trace §7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_structs,
    reason = "constructed field-by-field by macro-generated code (like `OpMeta`); pinning every field is intentional"
)]
pub struct SpanEvent {
    /// The CTSC event name.
    pub name: EventName,
    /// The event's attributes (e.g. `conformance.observation.value`).
    pub attributes: BTreeMap<String, Value>,
}

/// One OTLP-shaped span in a capture (CTSC trace §6).
///
/// Ids and tick timestamps are deterministic and used for nesting/order only;
/// they are never compared across runs (see the module docs). The `Debug`
/// output is a trust-anchor oracle, and equality is strict and structural
/// (matching [`crate::WatchEvent`]).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_structs,
    reason = "constructed field-by-field by macro-generated code (like `OpMeta`); pinning every field is intentional"
)]
pub struct Span {
    /// OTLP-native trace id, shared by every span in one capture. Nesting only;
    /// never compared across runs.
    pub trace_id: [u8; 16],
    /// OTLP-native span id. Nesting only; never compared across runs.
    pub span_id: [u8; 8],
    /// The parent span's id, or `None` for a root span.
    pub parent_span_id: Option<[u8; 8]>,
    /// The CTSC span name.
    pub name: SpanName,
    /// Monotonic open tick. Ordering/containment only; never compared across runs.
    pub start: u64,
    /// Monotonic close tick, set on `SpanGuard` drop. Ordering only.
    pub end: Option<u64>,
    /// The span's attributes (e.g. `conformance.operation.inputs`).
    pub attributes: BTreeMap<String, Value>,
    /// The span's events, in emission order.
    pub events: Vec<SpanEvent>,
}

/// One frame on the current-span stack. Each [`open_span`] frame carries the
/// buffer index of its span so `Drop` can stamp its `end`.
#[derive(Debug, Clone, Copy)]
struct Frame {
    span_id: [u8; 8],
    buffer_index: usize,
}

thread_local! {
    /// Spans opened on this thread, in open order.
    static SPANS: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
    /// The current-span stack: parents of the span being opened next.
    static STACK: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    /// The trace id shared by this capture, minted on `reset` / first span.
    static TRACE_ID: Cell<Option<[u8; 16]>> = const { Cell::new(None) };
    // TODO(#42): id/tick counters are per-thread. Cross-thread (parallel) span
    // emission needs globally-unique (lane-encoded) span_ids so a worker's spans
    // don't collide with the parent's within one trace.
    /// Monotonic span-id counter.
    static SPAN_ID_COUNTER: Cell<u64> = const { Cell::new(0) };
    /// Monotonic clock/tick counter (advances on open and close).
    static TICK_COUNTER: Cell<u64> = const { Cell::new(0) };
    /// Monotonic trace-id counter; a fresh value is minted on `reset`.
    static TRACE_ID_COUNTER: Cell<u64> = const { Cell::new(0) };
}

/// Mint the next span id as a big-endian counter.
fn next_span_id() -> [u8; 8] {
    let n = SPAN_ID_COUNTER.with(|c| {
        let next = c.get() + 1;
        c.set(next);
        next
    });
    n.to_be_bytes()
}

/// Advance and return the clock/tick counter.
fn next_tick() -> u64 {
    TICK_COUNTER.with(|c| {
        let next = c.get() + 1;
        c.set(next);
        next
    })
}

/// Mint a fresh trace id as a big-endian counter in the low 8 bytes.
fn mint_trace_id() -> [u8; 16] {
    let n = TRACE_ID_COUNTER.with(|c| {
        let next = c.get() + 1;
        c.set(next);
        next
    });
    let mut id = [0_u8; 16];
    id[8..].copy_from_slice(&n.to_be_bytes());
    id
}

/// Return the current trace id, minting one lazily if none exists yet (the
/// initial state before any [`reset`]).
fn current_trace_id() -> [u8; 16] {
    TRACE_ID.with(|t| {
        if let Some(id) = t.get() {
            id
        } else {
            let id = mint_trace_id();
            t.set(Some(id));
            id
        }
    })
}

/// Open a new span as a child of the current stack top (or a root if the stack
/// is empty), push it onto the buffer and the current-span stack, and return a
/// [`SpanGuard`] that closes it on drop.
///
/// Events pushed with [`push_event`] before the guard drops land on this span.
#[must_use = "dropping the guard immediately closes the span with no body"]
pub fn open_span(name: SpanName, attributes: BTreeMap<String, Value>) -> SpanGuard {
    let trace_id = current_trace_id();
    let span_id = next_span_id();
    let parent_span_id = STACK.with(|s| s.borrow().last().map(|f| f.span_id));
    let start = next_tick();
    let span = Span {
        trace_id,
        span_id,
        parent_span_id,
        name,
        start,
        end: None,
        attributes,
        events: Vec::new(),
    };
    let index = SPANS.with(|b| {
        let mut b = b.borrow_mut();
        b.push(span);
        b.len() - 1
    });
    STACK.with(|s| {
        s.borrow_mut().push(Frame {
            span_id,
            buffer_index: index,
        });
    });
    SpanGuard {
        buffer_index: index,
    }
}

/// Append a [`SpanEvent`] to the current (stack-top) span, in emission order.
///
/// If no span is open this is a caller error; mirroring the flat buffer's
/// escape-hatch behavior (`buffer.rs`), the event is silently dropped rather
/// than panicking, since a stray emission outside any span has nowhere to land.
pub fn push_event(name: EventName, attributes: BTreeMap<String, Value>) {
    let Some(index) = STACK.with(|s| s.borrow().last().map(|f| f.buffer_index)) else {
        return;
    };
    SPANS.with(|b| {
        if let Some(span) = b.borrow_mut().get_mut(index) {
            span.events.push(SpanEvent { name, attributes });
        }
    });
}

/// Drain and return all buffered spans for the current thread, leaving the
/// buffer empty (parallels [`crate::take_events`]).
#[must_use]
pub fn take_spans() -> Vec<Span> {
    SPANS.with(|b| std::mem::take(&mut *b.borrow_mut()))
}

/// Clear the thread-local span buffer, current-span stack, and counters, and
/// mint a fresh `trace_id` for the next capture (parallels [`crate::reset`]).
#[allow(
    dead_code,
    reason = "additive span-buffer reset; wired into the macro path by slice #37b"
)]
pub fn reset() {
    SPANS.with(|b| b.borrow_mut().clear());
    STACK.with(|s| s.borrow_mut().clear());
    SPAN_ID_COUNTER.with(|c| c.set(0));
    TICK_COUNTER.with(|c| c.set(0));
    TRACE_ID.with(|t| t.set(Some(mint_trace_id())));
}

/// RAII guard returned by [`open_span`]. On drop it pops the current-span stack
/// and stamps the span's `end` tick.
#[derive(Debug)]
#[must_use = "a span stays open until its guard is dropped"]
pub struct SpanGuard {
    /// The buffer index of the span to close.
    buffer_index: usize,
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        STACK.with(|s| {
            s.borrow_mut().pop();
        });
        let end = next_tick();
        SPANS.with(|b| {
            if let Some(span) = b.borrow_mut().get_mut(self.buffer_index) {
                span.end = Some(end);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctsc_string_mapping() {
        assert_eq!(SpanName::Run.as_ctsc_str(), "conformance.run");
        assert_eq!(SpanName::Scenario.as_ctsc_str(), "conformance.scenario");
        assert_eq!(SpanName::Operation.as_ctsc_str(), "conformance.operation");

        assert_eq!(
            EventName::Observation.as_ctsc_str(),
            "conformance.observation"
        );
        assert_eq!(EventName::Result.as_ctsc_str(), "conformance.result");
        assert_eq!(EventName::Empty.as_ctsc_str(), "conformance.empty");
        assert_eq!(EventName::Error.as_ctsc_str(), "conformance.error");
        assert_eq!(EventName::Fault.as_ctsc_str(), "conformance.fault");
    }

    #[test]
    fn top_level_span_has_no_parent() {
        reset();
        let g = open_span(SpanName::Operation, BTreeMap::new());
        drop(g);
        let spans = take_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].parent_span_id, None);
        assert_eq!(spans[0].name, SpanName::Operation);
        assert!(spans[0].start < spans[0].end.expect("closed on drop"));
    }

    #[test]
    fn nested_span_links_to_parent_and_nests_intervals() {
        reset();
        let parent_guard = open_span(SpanName::Scenario, BTreeMap::new());
        let child_guard = open_span(SpanName::Operation, BTreeMap::new());
        drop(child_guard);
        drop(parent_guard);

        let spans = take_spans();
        assert_eq!(spans.len(), 2);
        let parent = &spans[0];
        let child = &spans[1];
        assert_eq!(parent.parent_span_id, None);
        assert_eq!(child.parent_span_id, Some(parent.span_id));

        let (ps, pe) = (parent.start, parent.end.expect("parent closed"));
        let (cs, ce) = (child.start, child.end.expect("child closed"));
        // parent.start < child.start < child.end < parent.end
        assert!(ps < cs && cs < ce && ce < pe, "intervals must nest");
    }

    #[test]
    fn sibling_spans_have_ordered_non_overlapping_intervals() {
        reset();
        drop(open_span(SpanName::Operation, BTreeMap::new()));
        drop(open_span(SpanName::Operation, BTreeMap::new()));
        let spans = take_spans();
        assert_eq!(spans.len(), 2);
        let a_end = spans[0].end.expect("a closed");
        let b_start = spans[1].start;
        assert!(spans[0].start < a_end);
        assert!(a_end < b_start, "siblings must not overlap");
        assert!(b_start < spans[1].end.expect("b closed"));
    }

    #[test]
    fn events_land_on_the_current_span_in_order() {
        reset();
        let outer_guard = open_span(SpanName::Scenario, BTreeMap::new());
        push_event(EventName::Observation, BTreeMap::new());

        let inner_guard = open_span(SpanName::Operation, BTreeMap::new());
        let mut attrs = BTreeMap::new();
        attrs.insert("conformance.result.value".to_string(), Value::Integer(5));
        push_event(EventName::Result, attrs);
        drop(inner_guard);

        push_event(EventName::Empty, BTreeMap::new());
        drop(outer_guard);

        let spans = take_spans();
        let outer = &spans[0];
        let inner = &spans[1];
        // Outer got the observation and, after the inner closed, the empty.
        assert_eq!(outer.events.len(), 2);
        assert_eq!(outer.events[0].name, EventName::Observation);
        assert_eq!(outer.events[1].name, EventName::Empty);
        // Inner got exactly its result.
        assert_eq!(inner.events.len(), 1);
        assert_eq!(inner.events[0].name, EventName::Result);
        assert_eq!(
            inner.events[0].attributes.get("conformance.result.value"),
            Some(&Value::Integer(5))
        );
    }

    #[test]
    fn push_event_outside_a_span_is_dropped() {
        reset();
        push_event(EventName::Observation, BTreeMap::new());
        assert!(take_spans().is_empty());
    }

    #[test]
    fn take_drains_and_all_spans_share_the_trace_id() {
        reset();
        drop(open_span(SpanName::Operation, BTreeMap::new()));
        drop(open_span(SpanName::Operation, BTreeMap::new()));
        let spans = take_spans();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].trace_id, spans[1].trace_id);
        // Draining leaves the buffer empty.
        assert!(take_spans().is_empty());
    }

    #[test]
    fn reset_mints_a_fresh_trace_id() {
        reset();
        drop(open_span(SpanName::Run, BTreeMap::new()));
        let first = take_spans()[0].trace_id;
        reset();
        drop(open_span(SpanName::Run, BTreeMap::new()));
        let second = take_spans()[0].trace_id;
        assert_ne!(first, second, "each capture gets a fresh trace_id");
    }

    #[test]
    fn debug_and_eq_round_trip() {
        reset();
        let g = open_span(SpanName::Operation, BTreeMap::new());
        push_event(EventName::Result, BTreeMap::new());
        drop(g);
        let spans = take_spans();
        let span = &spans[0];
        // Debug is the trust-anchor oracle: it must render the shape faithfully.
        let debug = format!("{span:?}");
        assert!(debug.contains("Operation"));
        assert!(debug.contains("Result"));
    }
}
