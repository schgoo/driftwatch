//! The [`DepObserve`] autoref-specialization ladder — the runtime disposition
//! decision for a `watch_dep!` observation.
//!
//! `value_emit.rs` only *encodes* a value; it makes no disposition decision, and
//! the `watch_dep!` macro has no type information about the wrapped call. This
//! ladder makes that decision at the concrete macro call site, dispatching on
//! the observed value's type and emitting the matching CTSC completion event as
//! a side effect (the macro then hands the value back unchanged):
//!
//! - `Result<T, E>` → `Ok`: [`push_result`](crate::push_result); `Err`:
//!   [`split_error`](crate::split_error) + [`push_error`](crate::push_error)
//!   over the error's `Display` string (fallback name `"error"` — a dep does
//!   not see `E`'s registry type).
//! - `Option<T>` → `Some`: `push_result`; `None`: [`push_empty`](crate::push_empty).
//! - plain `T` (incl. `()` and any non-`ToValue` structural value) →
//!   `push_result`.
//!
//! An `Ok`/`Some` value encodes via [`ToValue`](crate::ToValue) when available
//! (structural fidelity), else via its [`Debug`] representation. `Result` and
//! `Option` disposition regardless of whether the inner value is `ToValue`, so
//! an `Err` is never recorded as a success nor a `None` as a `result` — as long
//! as the inner value is `Debug`.
//!
//! # Why the encoding lives on the impls
//!
//! Autoref specialization only selects an impl at a site where the type is
//! concrete; a generic helper (`fn encode<T>(..)`) cannot pick the `ToValue`
//! level, since trait selection is fixed at the generic's definition. The
//! `watch_dep!` macro *is* that concrete site, so the encoding bounds
//! (`T: ToValue`, `E: Display`, …) sit on the [`DepObserve`] impls themselves
//! and resolve there.
//!
//! Method resolution mirrors the [`ValueEmit`](crate::ValueEmit) ladder's
//! autoref depth/priority discipline so both may be in scope at one `watch_dep!`
//! call site without conflicting impls. Most refs win, so the `ToValue` `Result`/
//! `Option` rungs shadow the `Debug`-bounded floor rungs, which shadow the
//! plain-value sub-ladder (per-`impl` docs below give exact depths). A call site
//! writes `(&&&&&&&DepObserve(&v)).observe()`.

use std::fmt::{Debug, Display};

use crate::{ToValue, Value, push_empty, push_error, push_result, split_error};

/// Wrapper around a borrowed observed value that drives the disposition ladder.
#[derive(Debug)]
#[expect(
    clippy::exhaustive_structs,
    reason = "a one-field newtype wrapper the watch_dep! macro constructs directly"
)]
pub struct DepObserve<'a, T: ?Sized>(
    /// The borrowed value whose disposition is observed.
    pub &'a T,
);

/// Emit an `Err` payload as a `conformance.error`: decompose the error's
/// `Display` string via [`split_error`](crate::split_error) under the fallback
/// name `"error"`, then [`push_error`](crate::push_error). Shared by both
/// `Result` rungs so the `Err` disposition is identical whether the `Ok`
/// payload is [`ToValue`](crate::ToValue) or only [`Debug`].
#[inline]
fn push_dep_error<E: Display>(e: &E) {
    let (name, value) = split_error(Value::String(format!("{e}")), "error");
    push_error(name, value);
}

/// Level 7 — `Result<T: ToValue, E>`: `Ok` → structural `result`, `Err` →
/// `error`.
pub trait DepObserveResult {
    /// Emit the disposition of the observed `Result`.
    fn observe(&self);
}

impl<T: ToValue, E: Display> DepObserveResult for &&&&&&&DepObserve<'_, Result<T, E>> {
    #[inline]
    fn observe(&self) {
        match self.0 {
            Ok(v) => push_result(v.to_value()),
            Err(e) => push_dep_error(e),
        }
    }
}

/// Level 6 — `Option<T: ToValue>`: `Some` → structural `result`, `None` →
/// `empty`.
pub trait DepObserveOption {
    /// Emit the disposition of the observed `Option`.
    fn observe(&self);
}

impl<T: ToValue> DepObserveOption for &&&&&&DepObserve<'_, Option<T>> {
    #[inline]
    fn observe(&self) {
        match self.0 {
            Some(v) => push_result(v.to_value()),
            None => push_empty(),
        }
    }
}

/// Level 5 — `Result<T: Debug, E>` floor: `Ok` → `Debug`-string `result`,
/// `Err` → `error`. Keeps `Ok`/`Err` disposition when the payload is not
/// [`ToValue`](crate::ToValue).
pub trait DepObserveResultDebug {
    /// Emit the disposition of the observed `Result` via the `Debug` floor.
    fn observe(&self);
}

impl<T: Debug, E: Display> DepObserveResultDebug for &&&&&DepObserve<'_, Result<T, E>> {
    #[inline]
    fn observe(&self) {
        match self.0 {
            Ok(v) => push_result(Value::String(format!("{v:?}"))),
            Err(e) => push_dep_error(e),
        }
    }
}

/// Level 4 — `Option<T: Debug>` floor: `Some` → `Debug`-string `result`,
/// `None` → `empty`. Keeps `Some`/`None` disposition when the payload is not
/// [`ToValue`](crate::ToValue).
pub trait DepObserveOptionDebug {
    /// Emit the disposition of the observed `Option` via the `Debug` floor.
    fn observe(&self);
}

impl<T: Debug> DepObserveOptionDebug for &&&&DepObserve<'_, Option<T>> {
    #[inline]
    fn observe(&self) {
        match self.0 {
            Some(v) => push_result(Value::String(format!("{v:?}"))),
            None => push_empty(),
        }
    }
}

/// Level 3 — plain [`ToValue`](crate::ToValue): a structural `result`.
pub trait DepObserveToValue {
    /// Emit the observed value as a structural `result`.
    fn observe(&self);
}

impl<T: ToValue + ?Sized> DepObserveToValue for &&&DepObserve<'_, T> {
    #[inline]
    fn observe(&self) {
        push_result(self.0.to_value());
    }
}

/// Level 2 — [`Display`]: a display-string `result`.
pub trait DepObserveDisplay {
    /// Emit the observed value's `Display` string as a `result`.
    fn observe(&self);
}

impl<T: Display + ?Sized> DepObserveDisplay for &&DepObserve<'_, T> {
    #[inline]
    fn observe(&self) {
        push_result(Value::String(format!("{}", self.0)));
    }
}

/// Level 1 — [`Debug`]: a debug-string `result` (the structural non-`ToValue`
/// safety net).
pub trait DepObserveDebug {
    /// Emit the observed value's `Debug` string as a `result`.
    fn observe(&self);
}

impl<T: Debug + ?Sized> DepObserveDebug for &DepObserve<'_, T> {
    #[inline]
    fn observe(&self) {
        push_result(Value::String(format!("{:?}", self.0)));
    }
}

/// Level 0 — any type: a type-name `result` (safety net).
pub trait DepObserveOther {
    /// Emit the observed value's type name as a `result`.
    fn observe(&self);
}

impl<T: ?Sized> DepObserveOther for DepObserve<'_, T> {
    #[inline]
    fn observe(&self) {
        push_result(Value::String(std::any::type_name::<T>().to_string()));
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::needless_borrow,
        reason = "uniform autoref-specialization call site mirrors macro output"
    )]

    use super::*;
    use crate::{EventName, SpanName, open_operation, reset, take_spans};
    use std::collections::BTreeMap;

    /// Open an operation and a nested dep span, observe `v` through the ladder,
    /// and return the dep span's single completion event.
    fn one_event<F: FnOnce()>(observe: F) -> crate::SpanEvent {
        reset();
        let op = open_operation("op", "c", BTreeMap::new());
        let dep = open_operation("dep", "c", BTreeMap::new());
        observe();
        drop(dep);
        drop(op);
        let spans = take_spans();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].name, SpanName::Operation);
        assert_eq!(spans[1].events.len(), 1);
        spans[1].events[0].clone()
    }

    #[test]
    fn result_ok_is_a_structural_result_event() {
        let ev = one_event(|| {
            let v: Result<i64, String> = Ok(7);
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ev.name, EventName::Result);
        assert_eq!(
            ev.attributes.get("conformance.result.value"),
            Some(&Value::Integer(7))
        );
    }

    #[test]
    fn result_err_is_an_error_event_with_fallback_name() {
        let ev = one_event(|| {
            let v: Result<i64, String> = Err("boom".to_string());
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ev.name, EventName::Error);
        assert_eq!(
            ev.attributes.get("conformance.error.name"),
            Some(&Value::String("error".to_string()))
        );
        assert_eq!(
            ev.attributes.get("conformance.error.value"),
            Some(&Value::String("boom".to_string()))
        );
    }

    #[test]
    fn option_some_is_a_result_and_none_is_empty() {
        let some = one_event(|| {
            let v: Option<i64> = Some(3);
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(some.name, EventName::Result);
        assert_eq!(
            some.attributes.get("conformance.result.value"),
            Some(&Value::Integer(3))
        );

        let none = one_event(|| {
            let v: Option<i64> = None;
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(none.name, EventName::Empty);
    }

    #[test]
    fn plain_to_value_is_a_structural_result_event() {
        let ev = one_event(|| {
            let v: i64 = 42;
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ev.name, EventName::Result);
        assert_eq!(
            ev.attributes.get("conformance.result.value"),
            Some(&Value::Integer(42))
        );
    }

    #[test]
    fn unit_value_is_a_result_via_the_debug_ladder() {
        let ev = one_event(|| {
            let v: () = ();
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ev.name, EventName::Result);
        assert_eq!(
            ev.attributes.get("conformance.result.value"),
            Some(&Value::String("()".to_string()))
        );
    }

    #[test]
    fn structural_non_to_value_falls_back_to_debug() {
        #[derive(Debug)]
        struct Opaque {
            #[expect(dead_code, reason = "read only via the derived Debug formatter")]
            code: u32,
        }
        let ev = one_event(|| {
            let v = Opaque { code: 9 };
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ev.name, EventName::Result);
        assert_eq!(
            ev.attributes.get("conformance.result.value"),
            Some(&Value::String("Opaque { code: 9 }".to_string()))
        );
    }

    /// A `Result` whose `Ok` payload is `Debug` but NOT `ToValue` must still
    /// disposition: `Ok` → `Result` (Debug-string value, not lost), `Err` →
    /// `Error` named `"error"` (never mis-recorded as a success).
    #[test]
    fn result_debug_floor_keeps_ok_err_disposition() {
        #[derive(Debug)]
        struct Opaque {
            #[expect(dead_code, reason = "read only via the derived Debug formatter")]
            code: u32,
        }

        let ok = one_event(|| {
            let v: Result<Opaque, String> = Ok(Opaque { code: 5 });
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(ok.name, EventName::Result);
        assert_eq!(
            ok.attributes.get("conformance.result.value"),
            Some(&Value::String("Opaque { code: 5 }".to_string()))
        );

        let err = one_event(|| {
            let v: Result<Opaque, String> = Err("boom".to_string());
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(err.name, EventName::Error);
        assert_eq!(
            err.attributes.get("conformance.error.name"),
            Some(&Value::String("error".to_string()))
        );
        assert_eq!(
            err.attributes.get("conformance.error.value"),
            Some(&Value::String("boom".to_string()))
        );
    }

    /// An `Option` whose payload is `Debug` but NOT `ToValue` must still
    /// disposition: `Some` → `Result` (Debug-string value), `None` → `Empty`.
    #[test]
    fn option_debug_floor_keeps_some_none_disposition() {
        #[derive(Debug)]
        struct Opaque {
            #[expect(dead_code, reason = "read only via the derived Debug formatter")]
            code: u32,
        }

        let some = one_event(|| {
            let v: Option<Opaque> = Some(Opaque { code: 8 });
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(some.name, EventName::Result);
        assert_eq!(
            some.attributes.get("conformance.result.value"),
            Some(&Value::String("Opaque { code: 8 }".to_string()))
        );

        let none = one_event(|| {
            let v: Option<Opaque> = None;
            (&&&&&&&DepObserve(&v)).observe();
        });
        assert_eq!(none.name, EventName::Empty);
    }
}
