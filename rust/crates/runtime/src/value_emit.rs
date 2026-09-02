//! The [`ValueEmit`] autoref-specialization ladder — the single value-encoding
//! ladder shared by the `conformance.result` and `conformance.error`
//! dispositions — plus the [`split_error`] name/value splitter.
//!
//! The ladder borrows a value and returns a [`Value`]; it makes no disposition
//! decision (the macro decides whether the encoding becomes a `result` or an
//! `error`). Method resolution picks the highest-priority trait the value
//! satisfies:
//!
//! - Level 1 — [`ToValue`]: `value.to_value()`.
//! - Level 2 — [`Display`](std::fmt::Display): `Value::String(format!("{x}"))`.
//! - Level 3 — [`Debug`](std::fmt::Debug): `Value::String(format!("{x:?}"))`.
//! - Level 4 — any type: `Value::String(type_name::<T>())`.
//!
//! Level 4 is a type-name safety net — unstable across toolchains and rarely
//! reached, since `Debug` catches almost everything. The autoref depth of each
//! impl target (`&&&`, `&&`, `&`, none) encodes priority: a call site writes
//! `(&&&&ValueEmit(&v)).encode()` and resolution selects the deepest satisfiable
//! level. Scalars at a `result`/`Ok`/`Some` disposition take the macro's direct
//! `ToValue` path and never reach this ladder.

use crate::{ToValue, Value};

/// Wrapper around a borrowed value that drives the encoding ladder.
#[derive(Debug)]
#[expect(
    clippy::exhaustive_structs,
    reason = "a one-field newtype wrapper the emit macros construct directly"
)]
pub struct ValueEmit<'a, T: ?Sized>(
    /// The borrowed value to encode.
    pub &'a T,
);

/// Level 1 — [`ToValue`]: structural encoding.
pub trait ValueEmitToValue {
    /// Encode via [`ToValue`].
    fn encode(&self) -> Value;
}

impl<T: ToValue + ?Sized> ValueEmitToValue for &&&ValueEmit<'_, T> {
    #[inline]
    fn encode(&self) -> Value {
        self.0.to_value()
    }
}

/// Level 2 — [`Display`](std::fmt::Display): a display string.
pub trait ValueEmitDisplay {
    /// Encode via [`Display`](std::fmt::Display).
    fn encode(&self) -> Value;
}

impl<T: std::fmt::Display + ?Sized> ValueEmitDisplay for &&ValueEmit<'_, T> {
    #[inline]
    fn encode(&self) -> Value {
        Value::String(format!("{}", self.0))
    }
}

/// Level 3 — [`Debug`](std::fmt::Debug): a debug string.
pub trait ValueEmitDebug {
    /// Encode via [`Debug`](std::fmt::Debug).
    fn encode(&self) -> Value;
}

impl<T: std::fmt::Debug + ?Sized> ValueEmitDebug for &ValueEmit<'_, T> {
    #[inline]
    fn encode(&self) -> Value {
        Value::String(format!("{:?}", self.0))
    }
}

/// Level 4 — any type: the type name (safety net).
pub trait ValueEmitUniversal {
    /// Encode as the type name of `T`.
    fn encode(&self) -> Value;
}

impl<T: ?Sized> ValueEmitUniversal for ValueEmit<'_, T> {
    #[inline]
    fn encode(&self) -> Value {
        Value::String(std::any::type_name::<T>().to_string())
    }
}

/// Split an encoded error [`Value`] into its `(error.name, error.value)` pair.
///
/// A [`Value::Variant`] yields `(tag, payload)` — the variant identifier is the
/// name. Any other value uses `fallback` as the name and keeps the value.
#[must_use]
pub fn split_error(value: Value, fallback: &str) -> (String, Value) {
    match value {
        Value::Variant { tag, value } => (tag, *value),
        other => (fallback.to_string(), other),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::needless_borrow,
        reason = "uniform autoref-specialization call site mirrors macro output"
    )]

    use super::*;
    use std::collections::BTreeMap;

    /// `ToValue` (a tagged variant) — Level 1.
    struct Tagged;
    impl ToValue for Tagged {
        fn to_value(&self) -> Value {
            Value::variant("Timeout", Value::Integer(30))
        }
    }

    /// `Display`-only — Level 2.
    struct Displayable;
    impl std::fmt::Display for Displayable {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "boom")
        }
    }

    /// `Debug`-only — Level 3.
    #[derive(Debug)]
    struct DebugOnly {
        #[expect(dead_code, reason = "read only via the derived Debug formatter")]
        code: u32,
    }

    /// None of the three — Level 4 (type-name safety net).
    struct Opaque;

    #[test]
    fn level_1_to_value_encodes_structurally() {
        let out = (&&&&ValueEmit(&Tagged)).encode();
        assert_eq!(out, Value::variant("Timeout", Value::Integer(30)));
    }

    #[test]
    fn level_2_display_encodes_display_string() {
        let out = (&&&&ValueEmit(&Displayable)).encode();
        assert_eq!(out, Value::String("boom".into()));
    }

    #[test]
    fn level_3_debug_encodes_debug_string() {
        let out = (&&&&ValueEmit(&DebugOnly { code: 7 })).encode();
        assert_eq!(out, Value::String("DebugOnly { code: 7 }".into()));
    }

    #[test]
    fn level_4_universal_encodes_type_name() {
        let out = (&&&&ValueEmit(&Opaque)).encode();
        // `type_name` output is not stable across compiler versions; only
        // require that it names the type.
        let Value::String(s) = out else {
            panic!("expected a string")
        };
        assert!(s.contains("Opaque"), "type name missing from {s:?}");
    }

    #[test]
    fn split_error_uses_variant_tag_as_name() {
        let (name, value) =
            split_error(Value::variant("NotFound", Value::Map(BTreeMap::new())), "E");
        assert_eq!(name, "NotFound");
        assert_eq!(value, Value::Map(BTreeMap::new()));
    }

    #[test]
    fn split_error_falls_back_for_non_variant() {
        let (name, value) = split_error(Value::String("divide by zero".into()), "String");
        assert_eq!(name, "String");
        assert_eq!(value, Value::String("divide by zero".into()));
    }
}
