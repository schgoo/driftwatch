//! Components, their dependencies, and registry imports (registry.md §3, §6, §7).

use serde::{Deserialize, Serialize};

use crate::named_type::NamedType;
use crate::operation::Operation;

/// A reference to another registry document (registry.md §6).
///
/// `imports` locate documents; the digest pins the exact imported bytes and the
/// URI is an optional retrieval hint. Loading imported documents and verifying
/// digests is deferred to the cross-document (Linked) resolution slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    /// The imported registry family id (§6).
    #[serde(rename = "registryId")]
    pub registry_id: String,
    /// The imported registry version (§6).
    pub version: String,
    /// `sha256:` digest pinning the exact imported document bytes (§6).
    pub digest: String,
    /// An optional retrieval hint for the imported document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// A component the owning component depends on (schema `componentRef`,
/// registry.md §7). A cross-component named-type reference requires a matching
/// dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// The depended-on component id (§7).
    #[serde(rename = "componentId")]
    pub component_id: String,
    /// The registry the component lives in, for cross-document dependencies (§7).
    #[serde(
        rename = "registryId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub registry_id: Option<String>,
}

/// A component: operations, named types, and declared dependencies, keyed by an
/// `id` unique across the resolved registry set (registry.md §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    /// The component id, unique across the resolved registry set (§3, §10).
    pub id: String,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The components this one depends on (§7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    /// The operation declarations.
    pub operations: Vec<Operation>,
    /// The named type declarations.
    pub types: Vec<NamedType>,
}
