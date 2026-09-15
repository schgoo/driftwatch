//! Component-level named type declarations (registry.md §5.6–§5.7, §3).
//!
//! A component's `types` array holds named records and tagged unions that its
//! operations reference by name. Unlike an inline [`crate::TypeRef`], a
//! `NamedType` always carries a `name` unique within its component (§10).

use serde::{Deserialize, Serialize};

use crate::type_ref::{Field, Variant};

/// A named type declared by a component (registry.md §5.6–§5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NamedType {
    /// A string-keyed record.
    Record {
        /// The type name, unique within the component (§10).
        name: String,
        /// The record fields.
        fields: Vec<Field>,
        /// An optional human-readable description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// A tagged union of named, optionally-payloaded variants.
    TaggedUnion {
        /// The type name, unique within the component (§10).
        name: String,
        /// The union variants.
        variants: Vec<Variant>,
        /// An optional human-readable description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

impl NamedType {
    /// The declared name, unique within the owning component (§10).
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            NamedType::Record { name, .. } | NamedType::TaggedUnion { name, .. } => name,
        }
    }
}
