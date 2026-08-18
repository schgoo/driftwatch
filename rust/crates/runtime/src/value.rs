//! The structured trace [`Value`] and its encoding.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Structured trace value: any scalar, or a `list`/`map`/`set` of values.
///
/// All integer widths canonicalize to `i64` and all float widths to `f64`, so
/// identical behavior compares equal across languages and versions; the precise
/// declared width lives in the contract, not here.
///
/// # Examples
///
/// ```
/// use runtime::Value;
///
/// let v = Value::List(vec![Value::Integer(1), Value::Bool(true)]);
/// assert_eq!(format!("{v:?}"), "List([Integer(1), Bool(true)])");
/// assert_ne!(Value::Integer(5), Value::Float(5.0));
/// ```
#[derive(Debug, Clone)]
#[expect(
    clippy::exhaustive_enums,
    reason = "core value lattice; a new variant must force exhaustive handling in every crate that matches it"
)]
pub enum Value {
    /// A UTF-8 string.
    String(String),
    /// A signed integer (all integer widths canonicalize here).
    Integer(i64),
    /// A floating-point number (all float widths canonicalize here).
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// An ordered list of values.
    List(Vec<Value>),
    /// A string-keyed map, kept in sorted key order for deterministic output.
    Map(BTreeMap<String, Value>),
    /// A set of values, kept sorted for deterministic output.
    Set(BTreeSet<Value>),
}

impl Value {
    /// Returns the canonical type name of this value.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Set(_) => "set",
        }
    }
}

/// Total-order rank for cross-variant comparison.
///
/// `Value` needs a total order because [`Value::Set`] stores a `BTreeSet` and
/// [`Value::Map`] a `BTreeMap`, both of which require `Ord`. Within a variant the
/// payload is compared directly; across variants this fixed rank decides the
/// order. The rank values are arbitrary but must stay stable: changing them
/// reorders every persisted `Set`/`Map` and would surface as spurious drift when
/// comparing captures written under different orderings.
fn variant_rank(value: &Value) -> u8 {
    match value {
        Value::Bool(_) => 0,
        Value::Integer(_) => 1,
        Value::Float(_) => 2,
        Value::String(_) => 3,
        Value::List(_) => 4,
        Value::Set(_) => 5,
        Value::Map(_) => 6,
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Value {
    /// Strict structural total order. Different variants never compare equal (an
    /// `Integer` is never equal to a `Float`, nor a `List` to a `Set`); floats
    /// order by [`f64::total_cmp`], so `NaN` is well-defined and `-0.0` differs
    /// from `0.0`. Equality is derived from this (`eq` is `cmp(..) == Equal`), so
    /// `Eq` and `Ord` always agree.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::List(a), Value::List(b)) => a.cmp(b),
            (Value::Map(a), Value::Map(b)) => a.cmp(b),
            (Value::Set(a), Value::Set(b)) => a.cmp(b),
            (a, b) => variant_rank(a).cmp(&variant_rank(b)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_float_edges() {
        assert_eq!(format!("{:?}", Value::Float(f64::NAN)), "Float(NaN)");
        assert_eq!(format!("{:?}", Value::Float(f64::INFINITY)), "Float(inf)");
        assert_eq!(
            format!("{:?}", Value::Float(f64::NEG_INFINITY)),
            "Float(-inf)"
        );
        assert_eq!(format!("{:?}", Value::Float(-0.0)), "Float(-0.0)");
    }

    #[test]
    fn debug_integer_edges() {
        assert_eq!(
            format!("{:?}", Value::Integer(i64::MIN)),
            "Integer(-9223372036854775808)"
        );
        assert_eq!(
            format!("{:?}", Value::Integer(i64::MAX)),
            "Integer(9223372036854775807)"
        );
    }

    #[test]
    fn debug_empty_collections() {
        assert_eq!(format!("{:?}", Value::List(Vec::new())), "List([])");
        assert_eq!(format!("{:?}", Value::Map(BTreeMap::new())), "Map({})");
        assert_eq!(format!("{:?}", Value::Set(BTreeSet::new())), "Set({})");
    }

    #[test]
    fn debug_nested_collections() {
        let mut map = BTreeMap::new();
        map.insert(
            "inner".to_string(),
            Value::List(vec![Value::Integer(1), Value::String("x".to_string())]),
        );

        assert_eq!(
            format!("{:?}", Value::List(vec![Value::Map(map)])),
            r#"List([Map({"inner": List([Integer(1), String("x")])})])"#
        );
    }

    #[test]
    fn variants_never_equal() {
        assert_ne!(Value::Integer(5), Value::Float(5.0));
        assert_ne!(
            Value::List(vec![Value::Integer(1)]),
            Value::Set(BTreeSet::from([Value::Integer(1)]))
        );
    }

    #[test]
    fn ordering_by_variant() {
        use std::cmp::Ordering;
        // Within a variant, ordered by payload.
        assert_eq!(Value::Integer(1).cmp(&Value::Integer(2)), Ordering::Less);
        assert_eq!(
            Value::String("a".into()).cmp(&Value::String("b".into())),
            Ordering::Less
        );
        // Across variants, ordered by rank: Bool < Integer < Float < String < List < Set < Map.
        assert_eq!(Value::Bool(true).cmp(&Value::Integer(0)), Ordering::Less);
        assert_eq!(Value::Integer(9).cmp(&Value::Float(0.0)), Ordering::Less);
        assert_eq!(
            Value::String("z".into()).cmp(&Value::List(vec![])),
            Ordering::Less
        );
        // Equal only when structurally identical.
        assert_eq!(Value::Integer(3).cmp(&Value::Integer(3)), Ordering::Equal);
        // The comparison operators (which route through `partial_cmp`) agree.
        assert!(Value::Integer(1) < Value::Integer(2));
        assert!(Value::Bool(true) < Value::Integer(0));
    }

    #[test]
    fn float_bits_equality() {
        assert_ne!(Value::Float(0.0), Value::Float(-0.0));
        assert_eq!(Value::Float(f64::NAN), Value::Float(f64::NAN));
    }

    #[test]
    fn type_names() {
        assert_eq!(Value::String(String::new()).type_name(), "string");
        assert_eq!(Value::Integer(0).type_name(), "int");
        assert_eq!(Value::Float(0.0).type_name(), "float");
        assert_eq!(Value::Bool(false).type_name(), "bool");
        assert_eq!(Value::List(Vec::new()).type_name(), "list");
        assert_eq!(Value::Map(BTreeMap::new()).type_name(), "map");
        assert_eq!(Value::Set(BTreeSet::new()).type_name(), "set");
    }

    #[test]
    fn debug_string_escaping() {
        let value = Value::String("quote: \", slash: \\, newline: \n, unicode: é 𝄞".to_string());
        assert_eq!(
            format!("{value:?}"),
            r#"String("quote: \", slash: \\, newline: \n, unicode: é 𝄞")"#
        );
    }
}
