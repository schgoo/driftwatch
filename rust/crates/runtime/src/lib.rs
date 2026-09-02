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
//! - [`Value`] — the universal structured value: any scalar, a tagged
//!   [`Value::Variant`], or a `list`/`map`/`set` of values. All integer widths
//!   canonicalize to `i64` and all float widths to `f64`.
//! - [`Span`] / [`SpanEvent`] — the OTLP-shaped CTSC span tree: a
//!   `conformance.operation` span carrying ordered completion/observation
//!   events (see [`span`]).
//! - [`ToValue`] — converts an annotated Rust value into a [`Value`]; this is
//!   the conversion the emit macros invoke.
//!
//! # Emission and discovery
//!
//! On top of the value type this crate provides the machinery the annotation
//! macros expand into: the thread-local span buffer and current-span stack
//! ([`open_operation`], [`push_observation`], [`push_result`], [`push_empty`],
//! [`push_error`], [`take_spans`], [`reset`]), the [`ValueEmit`]
//! autoref-specialization ladder — the single value-encoding ladder shared by
//! the `conformance.result` and `conformance.error` dispositions — plus
//! [`split_error`] for the error name/value split, and the link-time
//! operation/type registry ([`OpMeta`] /
//! [`TypeMeta`] via [`discovery_json`]) the extraction driver reads to derive a
//! contract. The buffer holds in-memory spans only — persisting a capture is
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
//! Equality and ordering are not decorative: [`Span`]/[`SpanEvent`] compare by
//! value, and the `Set` variant is stored in a `BTreeSet`, which requires
//! `Value` to be totally ordered.
//!
//! This crate defines only the in-memory span/value types and their comparison;
//! it does not serialize them. Persisting a capture to a file is the artifact
//! layer's job, and its format must round-trip these values faithfully —
//! notably preserving `Set` vs `List` and the float edge cases — so that a
//! re-read capture compares identically to the original. Relaxed, field-scoped
//! matching for nondeterministic data (timestamps, random ids) likewise belongs
//! in the comparison layer above this crate, never in `Value`'s own equality.
//!
//! # Examples
//!
//! ```
//! use runtime::{ToValue, Value};
//!
//! // Convert a Rust value into the canonical value...
//! assert_eq!(
//!     vec![1_i32, 2].to_value(),
//!     Value::List(vec![Value::Integer(1), Value::Integer(2)])
//! );
//!
//! // ...an `Option` projects to a tagged union.
//! assert_eq!(Some(5_i32).to_value(), Value::variant("Some", Value::Integer(5)));
//! ```

mod registry;
mod span;
mod to_value;
mod value;
mod value_emit;

pub use registry::{
    DRIFTWATCH_OPS, DRIFTWATCH_TYPES, FieldMeta, OpMeta, TypeMeta, VariantMeta, discovery_json,
};
pub use span::{
    EventName, Span, SpanEvent, SpanGuard, SpanName, open_operation, open_span, push_empty,
    push_error, push_event, push_observation, push_result, reset, take_spans,
};
pub use to_value::ToValue;
pub use value::Value;
pub use value_emit::{
    ValueEmit, ValueEmitDebug, ValueEmitDisplay, ValueEmitToValue, ValueEmitUniversal, split_error,
};

/// Re-exported for macro-generated code, which references `linkme` paths when
/// registering operations and types into the link-time registry.
pub use linkme;
