//! The [`ToValue`] conversion trait and its standard-library impls.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::Value;

/// Converts an annotated value into a structured [`Value`].
///
/// Implemented for the scalar types (integers, floats, `bool`, `char`, and the
/// string types), the standard collections (`Vec`, slices, arrays,
/// `BTreeMap`/`HashMap` keyed by `String`, `BTreeSet`/`HashSet`), `Option`,
/// references, and `Box`. All integers encode to [`Value::Integer`] (an `i64`):
/// values above `i64::MAX` (`u64`/`usize`) wrap, because the canonical lattice
/// has a single signed-integer kind. `Some(v)` encodes as `v` and `None` as an
/// empty string (see the [`Option`] impl for the ambiguity this carries).
///
/// # Examples
///
/// ```
/// use runtime::{ToValue, Value};
///
/// assert_eq!(42_i32.to_value(), Value::Integer(42));
/// assert_eq!("hi".to_value(), Value::String("hi".into()));
/// ```
pub trait ToValue {
    /// Returns the [`Value`] encoding of `self`.
    fn to_value(&self) -> Value;
}

macro_rules! impl_ints {
    ($($t:ty),*) => {
        $(impl ToValue for $t {
            fn to_value(&self) -> Value { Value::Integer(i64::from(*self)) }
        })*
    };
}
impl_ints!(i8, i16, i32, u8, u16, u32);

impl ToValue for isize {
    fn to_value(&self) -> Value {
        Value::Integer(*self as i64)
    }
}

impl ToValue for u64 {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "values above i64::MAX are not expected in traces; wrap is accepted for the canonical i64 lattice"
    )]
    fn to_value(&self) -> Value {
        Value::Integer(*self as i64)
    }
}

impl ToValue for usize {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "values above i64::MAX are not expected in traces; wrap is accepted for the canonical i64 lattice"
    )]
    fn to_value(&self) -> Value {
        Value::Integer(*self as i64)
    }
}

impl ToValue for i64 {
    fn to_value(&self) -> Value {
        Value::Integer(*self)
    }
}

impl ToValue for f32 {
    fn to_value(&self) -> Value {
        Value::Float(f64::from(*self))
    }
}
impl ToValue for f64 {
    fn to_value(&self) -> Value {
        Value::Float(*self)
    }
}
impl ToValue for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}
impl ToValue for char {
    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}
impl ToValue for str {
    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}
impl ToValue for String {
    fn to_value(&self) -> Value {
        Value::String(self.clone())
    }
}

impl<T: ToValue> ToValue for Vec<T> {
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(ToValue::to_value).collect())
    }
}
impl<T: ToValue> ToValue for [T] {
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(ToValue::to_value).collect())
    }
}
impl<T: ToValue, const N: usize> ToValue for [T; N] {
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(ToValue::to_value).collect())
    }
}
impl<T: ToValue> ToValue for BTreeMap<String, T> {
    fn to_value(&self) -> Value {
        Value::Map(
            self.iter()
                .map(|(k, v)| (k.clone(), v.to_value()))
                .collect(),
        )
    }
}
impl<T: ToValue, S: std::hash::BuildHasher> ToValue for HashMap<String, T, S> {
    fn to_value(&self) -> Value {
        Value::Map(
            self.iter()
                .map(|(k, v)| (k.clone(), v.to_value()))
                .collect(),
        )
    }
}
impl<T: ToValue + Ord> ToValue for BTreeSet<T> {
    fn to_value(&self) -> Value {
        Value::Set(self.iter().map(ToValue::to_value).collect())
    }
}
impl<T: ToValue + Eq + std::hash::Hash, S: std::hash::BuildHasher> ToValue for HashSet<T, S> {
    fn to_value(&self) -> Value {
        let mut v: Vec<Value> = self.iter().map(ToValue::to_value).collect();
        v.sort();
        Value::Set(v.into_iter().collect())
    }
}

impl<T: ToValue> ToValue for Option<T> {
    /// `Some(v)` encodes as `v`; `None` encodes as an empty string.
    ///
    /// The empty-string sentinel for `None` is ambiguous with `Some("")` and
    /// with any other value that encodes to `String("")`. Field emission avoids
    /// this by representing an absent optional field as the *absence* of its
    /// event rather than an empty value; this direct encoding is the fallback
    /// for an `Option` used as a value.
    fn to_value(&self) -> Value {
        match self {
            Some(v) => v.to_value(),
            None => Value::String(String::new()),
        }
    }
}

impl ToValue for Value {
    fn to_value(&self) -> Value {
        self.clone()
    }
}

impl<T: ToValue + ?Sized> ToValue for &T {
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}
impl<T: ToValue + ?Sized> ToValue for Box<T> {
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(3_u8.to_value(), Value::Integer(3));
        assert_eq!((-2_i64).to_value(), Value::Integer(-2));
        assert_eq!(1.5_f64.to_value(), Value::Float(1.5));
        assert_eq!(true.to_value(), Value::Bool(true));
        assert_eq!("hi".to_value(), Value::String("hi".into()));
        assert_eq!('c'.to_value(), Value::String("c".into()));
    }

    #[test]
    fn unsigned_boundaries() {
        assert_eq!(u32::MAX.to_value(), Value::Integer(i64::from(u32::MAX)));
        assert_eq!(u64::MAX.to_value(), Value::Integer(-1));
    }

    #[test]
    fn options_and_references() {
        assert_eq!(Some(4_i32).to_value(), Value::Integer(4));
        assert_eq!(None::<i32>.to_value(), Value::String(String::new()));
        assert_eq!(Box::new(6_i32).to_value(), Value::Integer(6));
    }

    #[test]
    fn collections() {
        assert_eq!(
            vec![1_i32, 2].to_value(),
            Value::List(vec![Value::Integer(1), Value::Integer(2)])
        );

        let mut map = BTreeMap::new();
        map.insert("k".to_string(), 1_i32);
        assert_eq!(
            map.to_value(),
            Value::Map(BTreeMap::from([("k".to_string(), Value::Integer(1))]))
        );

        let set = BTreeSet::from([2_i32, 1]);
        assert_eq!(
            set.to_value(),
            Value::Set(BTreeSet::from([Value::Integer(1), Value::Integer(2)]))
        );
    }
}
