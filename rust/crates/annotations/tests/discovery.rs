//! The discovery envelope: registered operations (with params/return) alongside
//! registered types.
//!
//! Feature-matrix coverage: `discovery_json()` output shape/ordering and JSON
//! validity. This crate registers exactly one operation and no types, so the
//! output is fully deterministic — the test locks the entire string. The
//! `component` is the author-declared literal (`annotations.compute`), which is
//! deliberately distinct from the crate name so the golden proves the component
//! source moved off `CARGO_PKG_NAME`.

use annotations::watch_operation;

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::needless_pass_by_value,
        reason = "identity (trace-off) form only borrows `b`; the shape exercises a String param"
    )
)]
#[watch_operation(component = "annotations.compute")]
fn compute(a: i64, b: String) -> i64 {
    let _ = &b;
    a
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn discovery_json_is_exact_and_valid() {
    let json = annotations::__rt::discovery_json();

    // Reference the annotated fn so it is not dead code when `#[watch_operation]`
    // expands to identity (trace off); discovery reads link-time metadata, not
    // the fn itself, so it is never otherwise called.
    let _ = compute as fn(i64, String) -> i64;

    // Exact string: locks ordering and content for the single registered op.
    assert_eq!(
        json,
        r#"{"operations":[{"name":"compute","module_path":"discovery","fn_name":"compute","is_setup":false,"is_async":false,"return_type":"i64","fills":"","component":"annotations.compute","params":[["a","i64"],["b","String"]]}],"types":[]}"#
    );

    // Also prove it parses as valid JSON and navigates as expected.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["operations"][0]["name"], "compute");
    assert_eq!(parsed["operations"][0]["component"], "annotations.compute");
    assert_eq!(parsed["operations"][0]["params"][1][0], "b");
    assert!(parsed["types"].as_array().unwrap().is_empty());
}
