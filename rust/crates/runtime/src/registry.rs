//! The link-time operation/type registry and its dependency-free JSON
//! discovery form.
//!
//! Annotation macros register each operation and `Watchable` type into the
//! [`DRIFTWATCH_OPS`] / [`DRIFTWATCH_TYPES`] distributed slices at link time;
//! [`discovery_json`] serializes the collected metadata into a JSON string the
//! extraction driver reads to derive a contract from annotated code.
//!
//! [`discovery_json`] produces an internal, dependency-free discovery handoff
//! consumed by the separate contract-extraction driver — it is NOT the persisted
//! capture artifact format (that remains deferred to issue #11), and whether the
//! driver consumes serialized JSON or reads the `&'static` metadata in-process is
//! a consumer-side decision for the extraction driver (issue #10).

use std::fmt::Write as _;

/// Metadata about one annotated operation or setup.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::exhaustive_structs,
    reason = "constructed field-by-field by external macro-generated code, which pins every field"
)]
pub struct OpMeta {
    /// The operation name (its spec identity).
    pub name: &'static str,
    /// The module/namespace path the operation is defined in.
    pub module_path: &'static str,
    /// The name of the annotated function.
    pub fn_name: &'static str,
    /// Whether this entry is a setup (`#[watch_setup]`) rather than an operation.
    pub is_setup: bool,
    /// Whether the annotated function is `async`.
    pub is_async: bool,
    /// The operation's parameters, as `(name, stringified-type)` pairs.
    pub params: &'static [FieldMeta],
    /// The stringified return type.
    pub return_type: &'static str,
    /// For setups: the operation parameter this setup fills (empty if unset).
    /// Used to disambiguate when several params share the setup's output type.
    pub fills: &'static str,
    /// The component (declared via `watch_component!` or a per-item `watch = "…"`
    /// override) that owns this operation. Extraction groups by component and
    /// derives cross-component `depends_on` from it.
    pub component: &'static str,
}

/// One named field with its (stringified) Rust type. Used for both operation
/// parameters and `Watchable` struct/enum-variant fields.
pub type FieldMeta = (&'static str, &'static str);

/// One enum variant: its name plus any named fields. Tuple and unit variants
/// carry an empty field list (schema extraction maps them to `{}`).
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::exhaustive_structs,
    reason = "constructed field-by-field by external macro-generated code, which pins every field"
)]
pub struct VariantMeta {
    /// The variant name.
    pub name: &'static str,
    /// The variant's named fields (empty for tuple/unit variants).
    pub fields: &'static [FieldMeta],
}

/// Metadata about a struct/enum that derives `Watchable`.
///
/// `kind` is `"struct"` or `"enum"`. Structs populate `fields` (only
/// `#[watchable]`-tagged fields, honoring `#[watchable(name = "…")]`); enums
/// populate `variants`.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::exhaustive_structs,
    reason = "constructed field-by-field by external macro-generated code, which pins every field"
)]
pub struct TypeMeta {
    /// The type name.
    pub name: &'static str,
    /// The module/namespace path the type is defined in.
    pub module_path: &'static str,
    /// Either `"struct"` or `"enum"`.
    pub kind: &'static str,
    /// The struct's fields (empty for enums).
    pub fields: &'static [FieldMeta],
    /// The enum's variants (empty for structs).
    pub variants: &'static [VariantMeta],
    /// The component that owns this type (see [`OpMeta::component`]).
    pub component: &'static str,
}

/// Link-time registry of annotated operations, populated across compilation
/// units and iterated by [`discovery_json`].
#[linkme::distributed_slice]
pub static DRIFTWATCH_OPS: [OpMeta];

/// Link-time registry of `Watchable`-deriving types, populated across
/// compilation units and iterated by [`discovery_json`].
#[linkme::distributed_slice]
pub static DRIFTWATCH_TYPES: [TypeMeta];

/// Escape a string for inclusion as a JSON string literal. Handles the control
/// and structural characters that can appear in stringified Rust types (quotes,
/// backslashes); other characters pass through. Kept dependency-free so the
/// runtime stays lean.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Write a JSON array of `[name, type]` field pairs into `out`.
fn write_fields_json(out: &mut String, fields: &[FieldMeta]) {
    out.push('[');
    for (i, (name, ty)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "[\"{}\",\"{}\"]", json_escape(name), json_escape(ty));
    }
    out.push(']');
}

/// Collect all registered metadata as JSON (used by the discovery binary).
///
/// The result is hand-rolled, dependency-free JSON with `operations` and
/// `types` arrays.
#[must_use]
pub fn discovery_json() -> String {
    let mut out = String::from("{\"operations\":[");
    for (i, op) in DRIFTWATCH_OPS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"name\":\"{}\",\"module_path\":\"{}\",\"fn_name\":\"{}\",\"is_setup\":{},\"is_async\":{},\"return_type\":\"{}\",\"fills\":\"{}\",\"component\":\"{}\",\"params\":",
            json_escape(op.name),
            json_escape(op.module_path),
            json_escape(op.fn_name),
            op.is_setup,
            op.is_async,
            json_escape(op.return_type),
            json_escape(op.fills),
            json_escape(op.component),
        );
        write_fields_json(&mut out, op.params);
        out.push('}');
    }
    out.push_str("],\"types\":[");
    for (i, ty) in DRIFTWATCH_TYPES.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"name\":\"{}\",\"module_path\":\"{}\",\"kind\":\"{}\",\"component\":\"{}\",\"fields\":",
            json_escape(ty.name),
            json_escape(ty.module_path),
            json_escape(ty.kind),
            json_escape(ty.component),
        );
        write_fields_json(&mut out, ty.fields);
        out.push_str(",\"variants\":[");
        for (j, v) in ty.variants.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let _ = write!(out, "{{\"name\":\"{}\",\"fields\":", json_escape(v.name));
            write_fields_json(&mut out, v.fields);
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_is_valid_json_with_expected_keys() {
        // Nothing is registered in the runtime's own test binary, so both
        // arrays are empty — but the envelope shape must still hold.
        let json = discovery_json();
        assert_eq!(json, "{\"operations\":[],\"types\":[]}");
        assert!(json.contains("\"operations\":"));
        assert!(json.contains("\"types\":"));
    }

    #[test]
    fn json_escape_handles_structural_and_control_chars() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("x\ny\tz"), "x\\ny\\tz");
        assert_eq!(json_escape("\u{0001}"), "\\u0001");
    }

    #[test]
    fn write_fields_json_emits_name_type_pairs() {
        let mut out = String::new();
        write_fields_json(&mut out, &[("a", "i32"), ("b", "String")]);
        assert_eq!(out, "[[\"a\",\"i32\"],[\"b\",\"String\"]]");
    }
}
