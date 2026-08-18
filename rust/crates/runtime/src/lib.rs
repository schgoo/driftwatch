//! Structured trace value and event types for the Driftwatch emitter.
//!
//! [`Value`] is the universal structured trace payload (scalars plus
//! `list`/`map`/`set` collections). [`TraceEvent`] is one emitted record.
//! [`ToValue`] converts an annotated value into a [`Value`].
//!
//! # Examples
//!
//! ```
//! use driftwatch_runtime::{ToValue, TraceEvent, Value};
//!
//! // Convert a Rust value into the canonical trace value...
//! assert_eq!(vec![1_i32, 2].to_value(), Value::List(vec![Value::Integer(1), Value::Integer(2)]));
//!
//! // ...and wrap it in an emitted event.
//! let event = TraceEvent::Event { name: "$result".into(), value: 5_i32.to_value() };
//! assert_eq!(event.name(), "$result");
//! ```
//!
//! Comparison is strict and structural: [`Value`] equality distinguishes every
//! variant (an `Integer` never equals a `Float`, nor a `List` a `Set`) and is
//! consistent with its ordering, so identical observations compare equal and any
//! difference is a real difference. Relaxed, field-scoped matching for
//! nondeterministic data (timestamps, random ids) is a separate concern applied
//! by the diff layer, not built into `Value`.
//!
//! Do not compare values via their serde form, which is lossy: a `Set`
//! serializes identically to a `List`, and `NaN`/`Inf` are not representable in
//! JSON. The serde encoding is for persistence, not equality.

mod to_value;
mod trace_event;
mod value;

pub use to_value::ToValue;
pub use trace_event::TraceEvent;
pub use value::Value;
