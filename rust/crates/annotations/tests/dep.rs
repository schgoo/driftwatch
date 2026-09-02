//! `#[watch_dep("name", component = "...")]`: a nested `conformance.operation`
//! span (own inputs + completion) around a real dependency call inside a
//! `#[watch_operation]` body.
//!
//! The observed call is `i64::from_str_radix` (an unannotated std function);
//! each argument keys the child span's inputs by identifier (`text`) or
//! positionally (`arg1` for the `16` literal).
//!
//! Coverage:
//! - `Ok`: child span carries a `conformance.result` (unwrapped);
//! - `Err`: child span carries a `conformance.error` (fallback name `"error"`,
//!   Display value — a dep does not see `E`'s type);
//! - trailing `?`: the child span closes before the `?` unwraps, then the op
//!   propagates;
//! - component: a dep inherits the enclosing operation's component unless it
//!   declares an override.

mod common;

use annotations::{SpanName, Value, reset, take_spans, watch_operation};
use common::{error, op_attrs, result};

#[watch_operation(component = "annotations")]
fn to_int(text: &str) -> i64 {
    #[watch_dep("parse")]
    let parsed = i64::from_str_radix(text, 16);
    parsed.unwrap_or(-1)
}

#[watch_operation(component = "annotations")]
fn to_int_try(text: &str) -> Result<i64, std::num::ParseIntError> {
    #[watch_dep("parse")]
    let n = i64::from_str_radix(text, 16)?;
    Ok(n)
}

#[watch_operation(component = "annotations")]
fn to_int_scoped(text: &str) -> i64 {
    #[watch_dep("parse", component = "annotations.parse")]
    let parsed = i64::from_str_radix(text, 16);
    parsed.unwrap_or(-1)
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_opens_a_nested_span_linked_to_the_parent() {
    reset();
    assert_eq!(to_int("2a"), 42);
    let spans = take_spans();
    assert_eq!(spans.len(), 2);
    let (parent, child) = (&spans[0], &spans[1]);
    assert_eq!(parent.name, SpanName::Operation);
    assert_eq!(child.name, SpanName::Operation);
    assert_eq!(child.parent_span_id, Some(parent.span_id));
    assert_eq!(
        parent.attributes,
        op_attrs(
            "annotations",
            "to_int",
            &[("text", Value::String("2a".into()))]
        )
    );
    // The dep inherits the parent component; args keyed by identifier / position.
    assert_eq!(
        child.attributes,
        op_attrs(
            "annotations",
            "parse",
            &[
                ("text", Value::String("2a".into())),
                ("arg1", Value::Integer(16)),
            ]
        )
    );
    assert_eq!(child.events, vec![result(42)]);
    assert_eq!(parent.events, vec![result(42)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_err_records_error_on_the_child_span() {
    reset();
    assert_eq!(to_int("zz"), -1);
    // Captured from the dependency itself so the assertion locks *what the dep
    // recorded* without coupling to libstd's exact wording.
    let dep_err = i64::from_str_radix("zz", 16).unwrap_err().to_string();
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    // A dep cannot see `E`'s type, so the error name is the fallback `"error"`.
    assert_eq!(child.events, vec![error("error", dep_err)]);
    // `to_int` returns the `unwrap_or(-1)` scalar.
    assert_eq!(parent.events, vec![result(-1)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_ok_with_try_propagates_and_both_spans_carry_result() {
    reset();
    assert_eq!(to_int_try("2a"), Ok(42));
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    assert_eq!(child.events, vec![result(42)]);
    assert_eq!(parent.events, vec![result(42)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_err_with_try_records_child_error_then_op_error() {
    reset();
    to_int_try("zz").unwrap_err();
    let dep_err = i64::from_str_radix("zz", 16).unwrap_err().to_string();
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    // The child span records the dep's error (fallback name), then the `?`
    // propagates and the op boundary records its own structural error, whose
    // fallback name is the last segment of `E` (`ParseIntError`).
    assert_eq!(child.events, vec![error("error", dep_err.clone())]);
    assert_eq!(parent.events, vec![error("ParseIntError", dep_err)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_component_override_scopes_the_child_span() {
    reset();
    assert_eq!(to_int_scoped("2a"), 42);
    let spans = take_spans();
    let child = &spans[1];
    // The declared override wins over the inherited parent component.
    assert_eq!(
        child.attributes,
        op_attrs(
            "annotations.parse",
            "parse",
            &[
                ("text", Value::String("2a".into())),
                ("arg1", Value::Integer(16)),
            ]
        )
    );
}
