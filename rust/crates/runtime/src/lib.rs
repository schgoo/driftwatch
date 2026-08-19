//! The Driftwatch runtime: the low-level substrate that annotated code emits
//! behavioral events into.
//!
//! Driftwatch captures a program's observable behavior as a stream of watch
//! events and diffs two captures to detect version-to-version drift. This crate
//! is the bottom of that stack — the structured value type, the event record,
//! and the conversion trait the annotation macros expand into. The extraction
//! driver runs annotated code and collects what this crate emits; the diff
//! engine compares two such collections. Nothing here knows about diffing or
//! bindings; it only defines *what an event is made of*.
//!
//! # Key types
//!
//! - [`Value`] — the universal structured value: any scalar, or a
//!   `list`/`map`/`set` of values. All integer widths canonicalize to `i64` and
//!   all float widths to `f64`.
//! - [`WatchEvent`] — one emitted record: a named [`WatchEvent::Event`] carrying
//!   a value, or a [`WatchEvent::Run`] marking the start of an operation.
//! - [`ToValue`] — converts an annotated Rust value into a [`Value`]; this is
//!   the conversion the emit macros invoke.
//!
//! # Emission and discovery
//!
//! On top of the event types this crate provides the machinery the annotation
//! macros expand into: a thread-local event buffer ([`emit_event`],
//! [`emit_event_v`], [`emit_run`], [`take_events`], [`reset`]), the
//! [`Watchable`] field-emission trait, the [`ReturnEmit`]
//! autoref-specialization ladder for `$result`
//! emission, and the link-time operation/type registry ([`OpMeta`] /
//! [`TypeMeta`] via [`discovery_json`]) the extraction driver reads to derive a
//! contract. The buffer holds in-memory events only — persisting a capture is
//! the artifact layer's job (see below).
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
//! Equality and ordering are not decorative: [`WatchEvent`] compares by value,
//! and the `Set` variant is stored in a `BTreeSet`, which requires `Value` to be
//! totally ordered.
//!
//! This crate defines only the in-memory event types and their comparison; it
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
//! use runtime::{ToValue, Value, WatchEvent};
//!
//! // Convert a Rust value into the canonical value...
//! assert_eq!(
//!     vec![1_i32, 2].to_value(),
//!     Value::List(vec![Value::Integer(1), Value::Integer(2)])
//! );
//!
//! // ...and wrap it in an emitted event.
//! let event = WatchEvent::Event { name: "$result".into(), value: 5_i32.to_value() };
//! assert_eq!(event.name(), "$result");
//! ```

mod buffer;
mod registry;
mod return_emit;
mod to_value;
mod value;
mod watch_event;
mod watchable;

pub use buffer::{emit_event, emit_event_v, emit_run, reset, take_events};
pub use registry::{
    DRIFTWATCH_OPS, DRIFTWATCH_TYPES, FieldMeta, OpMeta, TypeMeta, VariantMeta, discovery_json,
};
pub use return_emit::{
    ReturnEmit, ReturnEmitDisplay, ReturnEmitNone, ReturnEmitStruct, ReturnEmitToValue,
};
pub use to_value::ToValue;
pub use value::Value;
pub use watch_event::WatchEvent;
pub use watchable::{Watchable, WatchableStruct};

/// Re-exported for macro-generated code, which references `linkme` paths when
/// registering operations and types into the link-time registry.
pub use linkme;
