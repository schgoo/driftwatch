//! Operation declarations: inputs, observations, and completion outcomes
//! (registry.md §4).

use serde::{Deserialize, Serialize};

use crate::type_ref::TypeRef;

/// A named, typed value — an operation input or observation (schema
/// `namedValue`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedValue {
    /// The value name, unique among its siblings (§10).
    pub name: String,
    /// The value type.
    #[serde(rename = "type")]
    pub ty: TypeRef,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A declared error outcome: a permitted `conformance.error.name` and optional
/// payload type (registry.md §4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorOutcome {
    /// The permitted `conformance.error.name`, unique per operation (§10).
    pub name: String,
    /// The optional decomposed error payload type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<TypeRef>,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The completion outcomes an operation may emit (registry.md §4.1).
///
/// `result` declares the success value type; `empty` permits
/// `conformance.empty` and requires `result`; `errors` enumerate the declared
/// error names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcomes {
    /// The successful semantic result type; absent means no result channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<TypeRef>,
    /// Whether `conformance.empty` is permitted; requires `result` (§4.1).
    #[serde(default, skip_serializing_if = "is_false")]
    pub empty: bool,
    /// The declared error outcomes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ErrorOutcome>,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if requires a &T predicate"
)]
fn is_false(b: &bool) -> bool {
    !*b
}

/// An operation declaration (registry.md §4): a semantic contract of inputs,
/// observations, and completion outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    /// The operation name, unique within the component (§10).
    pub name: String,
    /// An optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The ordered input declarations.
    pub inputs: Vec<NamedValue>,
    /// The observation declarations.
    pub observations: Vec<NamedValue>,
    /// The completion outcomes.
    pub outcomes: Outcomes,
}
