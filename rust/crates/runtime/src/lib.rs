//! The Driftwatch runtime: the low-level substrate that annotated code emits
//! behavioral traces into.
//!
//! Driftwatch captures a program's observable behavior as a stream of trace
//! events and diffs two captures to detect version-to-version drift. This crate
//! is the bottom of that stack — the structured value type, the event record,
//! and the conversion trait the annotation macros expand into. The extraction
//! driver runs annotated code and collects what this crate emits; the diff
//! engine compares two such collections. Nothing here knows about diffing or
//! bindings; it only defines *what a trace is made of*.
//!
//! # Key types
//!
//! - [`Value`] — the universal structured trace value: any scalar, or a
//!   `list`/`map`/`set` of values. All integer widths canonicalize to `i64` and
//!   all float widths to `f64`.
//! - [`TraceEvent`] — one emitted record: a named [`TraceEvent::Event`] carrying
//!   a value, or a [`TraceEvent::Run`] marking the start of an operation.
//! - [`ToValue`] — converts an annotated Rust value into a [`Value`]; this is
//!   the conversion the emit macros invoke.
//!
//! # Comparison
//!
//! [`Value`] implements strict, structural equality and a matching total order.
//! Two values are equal only when they have the same variant and the same
//! contents — an `Integer` never equals a `Float`, nor a `List` a `Set` — and
//! floats compare by their total ordering, so `NaN` equals itself and `-0.0`
//! differs from `0.0`. `eq` is defined as `cmp(..) == Equal`, so equality and
//! ordering can never disagree. Because equality is exact, "equal" means an
//! identical observation and any inequality is a genuine difference — the
//! property a drift check depends on.
//!
//! Equality and ordering are not decorative: [`TraceEvent`] compares by value,
//! and the `Set` variant is stored in a `BTreeSet`, which requires `Value` to be
//! totally ordered.
//!
//! This crate defines only the in-memory trace types and their comparison; it
//! does not serialize them. Persisting a capture to a file is the artifact
//! layer''s job, and its format must round-trip these values faithfully —
//! notably preserving `Set` vs `List` and the float edge cases — so that a
//! re-read capture compares identically to the original. Relaxed, field-scoped
//! matching for nondeterministic data (timestamps, random ids) likewise belongs
//! in the comparison layer above this crate, never in `Value`''s own equality.
//!
//! # Examples
//!
//! ```
//! use runtime::{ToValue, TraceEvent, Value};
//!
//! // Convert a Rust value into the canonical trace value...
//! assert_eq!(
//!     vec![1_i32, 2].to_value(),
//!     Value::List(vec![Value::Integer(1), Value::Integer(2)])
//! );
//!
//! // ...and wrap it in an emitted event.
//! let event = TraceEvent::Event { name: "$result".into(), value: 5_i32.to_value() };
//! assert_eq!(event.name(), "$result");
//! ```

mod to_value;
mod trace_event;
mod value;

pub use to_value::ToValue;
pub use trace_event::TraceEvent;
pub use value::Value;
