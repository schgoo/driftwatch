//! The caller-supplied CTSC resource descriptor.

use runtime::Value;

/// The CTSC conformance profile version this emitter targets. CTSC requires the
/// `conformance.version` resource attribute to equal this exact string.
pub const CONFORMANCE_VERSION: &str = "0.1.0";

/// The required CTSC Trace Core resource attributes, supplied by the caller.
///
/// These identify the producing tool and the target under observation; the
/// `artifact` crate deliberately hardcodes neither (a Rust and a C# run of the
/// same target declare the same `target.*`). `conformance.version` is fixed to
/// [`CONFORMANCE_VERSION`] and emitted automatically. The Linked-profile
/// `service.name` and `conformance.registry.*` attributes are out of scope for
/// Trace Core and are not emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    tool_name: String,
    tool_version: String,
    target_name: String,
    target_language: String,
}

impl Resource {
    /// Builds a resource descriptor from the required CTSC identity attributes:
    /// the tool name/version (the producer) and the target name/language (the
    /// implementation under observation).
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        tool_version: impl Into<String>,
        target_name: impl Into<String>,
        target_language: impl Into<String>,
    ) -> Resource {
        Resource {
            tool_name: tool_name.into(),
            tool_version: tool_version.into(),
            target_name: target_name.into(),
            target_language: target_language.into(),
        }
    }

    /// The ordered CTSC resource attributes as `(key, AnyValue)` pairs, with
    /// `conformance.version` injected. Every attribute is a string value.
    pub(crate) fn attributes(&self) -> Vec<(&'static str, Value)> {
        vec![
            (
                "conformance.version",
                Value::String(CONFORMANCE_VERSION.to_string()),
            ),
            (
                "conformance.tool.name",
                Value::String(self.tool_name.clone()),
            ),
            (
                "conformance.tool.version",
                Value::String(self.tool_version.clone()),
            ),
            (
                "conformance.target.name",
                Value::String(self.target_name.clone()),
            ),
            (
                "conformance.target.language",
                Value::String(self.target_language.clone()),
            ),
        ]
    }
}
