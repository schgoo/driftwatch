//! The [`ReturnEmit`] autoref-specialization ladder for `$result` emission.
//!
//! The emit macros wrap an operation's return value in [`ReturnEmit`] and call
//! `.emit_result()`. Method resolution picks the highest-priority trait whose
//! bound the return type satisfies, walking down a fixed ladder:
//!
//! - Level 1 — struct returns ([`SpecEventStruct`]): per-field events + a
//!   structured `$result`.
//! - Level 2 — enums / collections / any [`ToValue`]: a structured `$result`.
//! - Level 3 — any [`Display`](std::fmt::Display) value: a `Display`-string
//!   `$result`.
//! - Level 4 (universal fallback) — any return type: emits nothing.
//!
//! All four are TRAIT impls (never an inherent method) so an unsatisfied bound
//! falls through to the next level instead of hard-erroring. The Level 4
//! fallback ensures an annotated op whose return type implements none of the
//! higher traits still COMPILES, emitting no `$result`. The autoref depth of
//! each impl target (`&&&`, `&&`, `&`, none) encodes the priority: more
//! references bind tighter, so a call site writes one extra reference
//! (`(&&&&ReturnEmit(&v)).emit_result()`) and method resolution selects the
//! deepest satisfiable level.
//!
//! Scalars (`i32`/`String`/`&str`/`bool`/…) are handled by the macro directly
//! via the `Display` path and never reach this ladder, so a primitive's
//! [`ToValue`] impl does not shadow its intended `Display` formatting.

use crate::{SpecEventStruct, ToValue, Value, emit_event_v};

/// Wrapper around a borrowed return value that drives the emission ladder.
///
/// It drives the ladder via autoref specialization: call
/// [`emit_result`](ReturnEmitNone::emit_result) on a (multiply-referenced)
/// `ReturnEmit` to emit the operation's `$result`.
#[derive(Debug)]
#[expect(
    clippy::exhaustive_structs,
    reason = "a one-field newtype wrapper the emit macros construct directly"
)]
pub struct ReturnEmit<'a, T: ?Sized>(
    /// The borrowed return value to emit.
    pub &'a T,
);

/// Level 1 (highest priority) — struct returns: per-field events + `$result`.
pub trait ReturnEmitStruct {
    /// Emit the struct's per-field events and a structured `$result`.
    fn emit_result(&self);
}

impl<T: SpecEventStruct + ?Sized> ReturnEmitStruct for &&&ReturnEmit<'_, T> {
    #[inline]
    fn emit_result(&self) {
        self.0.emit_fields(None);
        emit_event_v("$result", self.0.to_value());
    }
}

/// Level 2 — enums / collections / any [`ToValue`]: structured `$result` only.
pub trait ReturnEmitToValue {
    /// Emit a structured `$result`.
    fn emit_result(&self);
}

impl<T: ToValue + ?Sized> ReturnEmitToValue for &&ReturnEmit<'_, T> {
    #[inline]
    fn emit_result(&self) {
        emit_event_v("$result", self.0.to_value());
    }
}

/// Level 3 — any [`Display`](std::fmt::Display) value: `Display`-string
/// `$result`.
pub trait ReturnEmitDisplay {
    /// Emit a `Display`-string `$result`.
    fn emit_result(&self);
}

impl<T: std::fmt::Display + ?Sized> ReturnEmitDisplay for &ReturnEmit<'_, T> {
    #[inline]
    fn emit_result(&self) {
        emit_event_v("$result", Value::String(format!("{}", self.0)));
    }
}

/// Level 4 (lowest priority, universal fallback) — any return type: emits
/// nothing.
pub trait ReturnEmitNone {
    /// Emit nothing (the fallback for return types satisfying no higher level).
    fn emit_result(&self);
}

impl<T: ?Sized> ReturnEmitNone for ReturnEmit<'_, T> {
    #[inline]
    fn emit_result(&self) {}
}

#[cfg(test)]
mod tests {
    // The ladder is exercised through the uniform `&&&&ReturnEmit(...)` call the
    // emit macros generate; for the lower levels the surplus reference is
    // immediately dereferenced, which `needless_borrow` flags. Keeping the call
    // uniform is the point of the test (resolution must pick the deepest
    // satisfiable level), so the lint is allowed here rather than hand-tuning
    // each call's depth. `allow` (not `expect`) because Level 1 consumes all
    // four references and does not fire.
    #![allow(
        clippy::needless_borrow,
        reason = "uniform autoref-specialization call site mirrors macro output"
    )]

    use super::*;
    use crate::{SpecEvent, TraceEvent, reset, take_traces};

    struct Wrapped(i64);

    impl SpecEvent for Wrapped {
        fn emit_fields(&self, prefix: Option<&str>) {
            let name = match prefix {
                Some(p) => format!("{p}.inner"),
                None => "inner".to_string(),
            };
            emit_event_v(&name, Value::Integer(self.0));
        }
    }
    impl ToValue for Wrapped {
        fn to_value(&self) -> Value {
            Value::Integer(self.0)
        }
    }
    impl SpecEventStruct for Wrapped {}

    /// A type that implements only `ToValue` (not `SpecEventStruct`, not
    /// `Display`) — should land on Level 2.
    struct OnlyValue;
    impl ToValue for OnlyValue {
        fn to_value(&self) -> Value {
            Value::Bool(true)
        }
    }

    /// A type that implements none of the ladder's traits — Level 4 fallback.
    struct Opaque;

    #[test]
    fn level_1_struct_emits_fields_and_result() {
        reset();
        let v = Wrapped(7);
        (&&&&ReturnEmit(&v)).emit_result();
        let traces = take_traces();
        assert_eq!(
            traces,
            vec![
                TraceEvent::Event {
                    name: "inner".into(),
                    value: Value::Integer(7),
                },
                TraceEvent::Event {
                    name: "$result".into(),
                    value: Value::Integer(7),
                },
            ]
        );
    }

    #[test]
    fn level_2_to_value_emits_structured_result() {
        reset();
        let v = OnlyValue;
        (&&&&ReturnEmit(&v)).emit_result();
        assert_eq!(
            take_traces(),
            vec![TraceEvent::Event {
                name: "$result".into(),
                value: Value::Bool(true),
            }]
        );
    }

    #[test]
    fn level_3_display_emits_string_result() {
        reset();
        let v = 42_u128;
        (&&&&ReturnEmit(&v)).emit_result();
        assert_eq!(
            take_traces(),
            vec![TraceEvent::Event {
                name: "$result".into(),
                value: Value::String("42".into()),
            }]
        );
    }

    #[test]
    fn level_4_fallback_emits_nothing() {
        reset();
        let v = Opaque;
        (&&&&ReturnEmit(&v)).emit_result();
        assert!(take_traces().is_empty());
    }
}
