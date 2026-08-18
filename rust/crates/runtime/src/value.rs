//! The structured trace [`Value`] and its encoding.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
/// use driftwatch_runtime::Value;
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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(s) => write!(f, "{s}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write_atom(f, v)?;
                }
                write!(f, "]")
            }
            Value::Set(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write_atom(f, v)?;
                }
                write!(f, "]")
            }
            Value::Map(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "\"{k}\":")?;
                    write_atom(f, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

fn write_atom(f: &mut std::fmt::Formatter<'_>, v: &Value) -> std::fmt::Result {
    match v {
        Value::String(s) => write!(f, "\"{s}\""),
        other => write!(f, "{other}"),
    }
}

// --- Serialize ------------------------------------------------------------

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::String(v) => serializer.serialize_str(v),
            Value::Integer(v) => serializer.serialize_i64(*v),
            Value::Float(v) => serializer.serialize_f64(*v),
            Value::Bool(v) => serializer.serialize_bool(*v),
            Value::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for it in items {
                    seq.serialize_element(it)?;
                }
                seq.end()
            }
            Value::Set(items) => {
                // Sets serialize as ordered arrays; a round-trip yields a
                // Value::List (see the module-level note on the lossy serde form).
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for it in items {
                    seq.serialize_element(it)?;
                }
                seq.end()
            }
            Value::Map(map) => {
                let mut m = serializer.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}

// --- Deserialize ----------------------------------------------------------

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;
impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("any JSON/YAML value")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Value::Integer(v))
    }
    #[expect(
        clippy::cast_possible_wrap,
        reason = "values above i64::MAX are not expected in traces; wrap is accepted for the canonical i64 lattice"
    )]
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Value::Integer(v as i64))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(Value::Float(v))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Value::String(v.to_string()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::String(String::new()))
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::String(String::new()))
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        Deserialize::deserialize(deserializer)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(v) = seq.next_element()? {
            out.push(v);
        }
        out.shrink_to_fit();
        Ok(Value::List(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut out = BTreeMap::new();
        while let Some((k, v)) = map.next_entry::<String, Value>()? {
            out.insert(k, v);
        }
        Ok(Value::Map(out))
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
    }

    #[test]
    fn float_bits_equality() {
        assert_ne!(Value::Float(0.0), Value::Float(-0.0));
        assert_eq!(Value::Float(f64::NAN), Value::Float(f64::NAN));
    }

    #[test]
    fn display_collections() {
        let v = Value::List(vec![Value::Integer(1), Value::String("x".into())]);
        assert_eq!(v.to_string(), r#"[1,"x"]"#);
        let mut m = BTreeMap::new();
        m.insert("k".to_string(), Value::Bool(true));
        assert_eq!(Value::Map(m).to_string(), r#"{"k":true}"#);
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
