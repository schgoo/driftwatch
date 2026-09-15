//! The registry document root and JSON parsing (registry.md §2).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::component::{Component, Import};

/// The `format` discriminant every CTSC registry document carries (§2).
pub const FORMAT: &str = "ctsc.registry";

/// The registry schema version this crate models (§2).
pub const FORMAT_VERSION: &str = "0.2.0";

/// A CTSC registry document: exactly one JSON object declaring one or more
/// components, optionally importing other documents (registry.md §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryDocument {
    /// The registry schema discriminant; must be [`FORMAT`].
    pub format: String,
    /// The registry schema version; must be [`FORMAT_VERSION`].
    #[serde(rename = "formatVersion")]
    pub format_version: String,
    /// The registry family id (§2).
    #[serde(rename = "registryId")]
    pub registry_id: String,
    /// The source package/release/commit/build the document was generated from (§2).
    pub version: String,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Other registry documents this one imports (§6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<Import>,
    /// The components this document declares (one or more) (§3).
    pub components: Vec<Component>,
}

impl RegistryDocument {
    /// Parse one registry document from JSON.
    ///
    /// This decodes the document shape only; call [`validate`](crate::validate)
    /// for the cross-item semantic rules (registry.md §4.1, §7, §8, §10).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the input is not a JSON registry document of
    /// the expected shape (missing required keys, wrong types, unknown `kind`).
    pub fn parse(json: &str) -> Result<Self, ParseError> {
        serde_json::from_str(json).map_err(ParseError)
    }
}

/// A failure to decode a registry document from JSON.
#[derive(Debug)]
pub struct ParseError(serde_json::Error);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid registry document: {}", self.0)
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
