//! CTSC Registry type references and the primitive scalar set (registry.md §5).
//!
//! A [`TypeRef`] is the recursive, language-neutral shape of any value the
//! registry declares — an input, observation, result, error payload, record
//! field, or union variant. It is internally tagged by `kind`, mirroring
//! `ctsc-registry-0.2.schema.json`.

use serde::{Deserialize, Serialize};

/// The CTSC primitive scalar types (registry.md §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Primitive {
    /// No value (`()` / `void`).
    Unit,
    /// A UTF-8 string.
    String,
    /// A boolean.
    Bool,
    /// A 32-bit signed integer.
    I32,
    /// A 64-bit signed integer.
    I64,
    /// A 32-bit unsigned integer.
    U32,
    /// A 64-bit unsigned integer.
    U64,
    /// A 32-bit float.
    F32,
    /// A 64-bit float.
    F64,
    /// An opaque byte string.
    Bytes,
}

/// A reference to a value type (registry.md §5).
///
/// `named` references resolve per registry.md §8; the composite kinds nest
/// further `TypeRef`s. Shape (`additionalProperties: false`, required keys) is
/// enforced by the JSON schema — this model captures the resolved variants so
/// registry validation can walk them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeRef {
    /// A primitive scalar (§5.1).
    Primitive {
        /// The scalar type.
        name: Primitive,
    },
    /// A reference to a named type, resolved by scope per §8.
    Named {
        /// The referenced type name.
        name: String,
        /// A component scope for the reference; absent means the current
        /// component (§8).
        #[serde(
            rename = "componentId",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        component_id: Option<String>,
        /// An imported-registry scope; requires `component_id` (§8).
        #[serde(
            rename = "registryId",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        registry_id: Option<String>,
    },
    /// An ordered collection (§5.3); position is part of the value.
    List {
        /// The element type.
        items: Box<TypeRef>,
    },
    /// An unordered, duplicate-free collection (§5.3).
    Set {
        /// The element type.
        items: Box<TypeRef>,
    },
    /// An explicitly-encoded optional: present or absent (§5, 0.2.0). Absence is
    /// encoded identically by every producer, so a bare present value is
    /// indistinguishable from a non-optional one.
    Optional {
        /// The wrapped value type.
        value: Box<TypeRef>,
    },
    /// A key/value mapping (§5.4).
    Map {
        /// The key type.
        keys: Box<TypeRef>,
        /// The value type.
        values: Box<TypeRef>,
    },
    /// A fixed-arity, positionally-typed value (§5.5).
    Tuple {
        /// The element type at each position.
        items: Vec<TypeRef>,
    },
    /// An inline record: string-keyed fields (§5.6).
    Record {
        /// The record fields.
        fields: Vec<Field>,
    },
    /// An inline tagged union: named, optionally-payloaded variants (§5.7).
    TaggedUnion {
        /// The union variants.
        variants: Vec<Variant>,
    },
}

/// A record field: a name bound to a type (registry.md §5.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// The field name, unique within the record (§10).
    pub name: String,
    /// The field type.
    #[serde(rename = "type")]
    pub ty: TypeRef,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A tagged-union variant: a name and an optional payload (registry.md §5.7).
///
/// Variant names carry no intrinsic CTSC meaning; type identity lives in the
/// registry, not the label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    /// The variant name, unique within the union (§10).
    pub name: String,
    /// The variant payload type, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<TypeRef>,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl TypeRef {
    /// Whether this is the `unit` primitive.
    #[must_use]
    pub fn is_unit(&self) -> bool {
        matches!(
            self,
            TypeRef::Primitive {
                name: Primitive::Unit
            }
        )
    }
}
