//! `watch_dep!("name", <expr>)`: a transparent observer that emits a nested
//! `conformance.operation` span (own inputs + completion) around a real
//! dependency call inside a `#[watch_operation]` body, returning the wrapped
//! value unchanged.
//!
//! The observed call is `i64::from_str_radix` (an unannotated std function);
//! each argument keys the child span's inputs by identifier (`text`) or
//! positionally (`arg1` for the `16` literal).
//!
//! Coverage:
//! - `Ok`: child span carries a `conformance.result` (unwrapped);
//! - `Err`: child span carries a `conformance.error` (fallback name `"error"`,
//!   Display value — a dep does not see `E`'s type);
//! - outside `?`: the child span closes before the `?` unwraps, then the op
//!   propagates;
//! - outside combinator (`.unwrap_or`/`.map`): exactly one dep span, value
//!   unchanged;
//! - component: a dep inherits the enclosing operation's component (runtime
//!   stack read) unless it declares an override;
//! - auto-inputs, non-call (zero inputs), and the value-ladder dispositions
//!   (`Option` Some→result / None→empty, plain-`T`→result, Unit dep, a
//!   structural non-`ToValue` value via the `Debug` fallback).

mod common;

use annotations::{SpanName, Value, reset, take_spans, watch_dep, watch_operation};
use common::{empty, error, op_attrs, result};

#[watch_operation(component = "annotations")]
fn to_int(text: &str) -> i64 {
    let parsed = watch_dep!("parse", i64::from_str_radix(text, 16));
    parsed.unwrap_or(-1)
}

#[watch_operation(component = "annotations")]
fn to_int_try(text: &str) -> Result<i64, std::num::ParseIntError> {
    let n = watch_dep!("parse", i64::from_str_radix(text, 16))?;
    Ok(n)
}

#[watch_operation(component = "annotations")]
fn to_int_scoped(text: &str) -> i64 {
    let parsed = watch_dep!(
        "parse",
        component = "annotations.parse",
        i64::from_str_radix(text, 16)
    );
    parsed.unwrap_or(-1)
}

/// Outside combinator: `.map_or(..)` composes on the returned value; exactly one
/// dep span is emitted and the value flows through unchanged.
#[watch_operation(component = "annotations")]
fn to_int_mapped(text: &str) -> i64 {
    watch_dep!("parse", i64::from_str_radix(text, 16)).map_or(-1, |n| n + 1)
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
fn dep_ok_with_outside_try_propagates_and_both_spans_carry_result() {
    reset();
    assert_eq!(to_int_try("2a"), Ok(42));
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    assert_eq!(child.events, vec![result(42)]);
    assert_eq!(parent.events, vec![result(42)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_err_with_outside_try_records_child_error_then_op_error() {
    reset();
    to_int_try("zz").unwrap_err();
    let dep_err = i64::from_str_radix("zz", 16).unwrap_err().to_string();
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    // The child span records the dep's error (fallback name) *before* the `?`
    // unwraps outside the macro, then the `?` propagates and the op boundary
    // records its own structural error (fallback name = last segment of `E`).
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

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn outside_combinator_emits_exactly_one_dep_span_and_passes_the_value() {
    reset();
    assert_eq!(to_int_mapped("2a"), 43);
    let spans = take_spans();
    // Exactly one dep span (plus the operation); the combinator runs outside.
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[1].events, vec![result(42)]);
    assert_eq!(spans[0].events, vec![result(43)]);
}

// ---------------------------------------------------------------------------
// Value-ladder dispositions + non-call / auto-input shapes.
// ---------------------------------------------------------------------------

fn opt_dep(n: i64) -> Option<i64> {
    (n > 0).then_some(n)
}

fn double(n: i64) -> i64 {
    n * 2
}

fn record(n: i64) {
    let _ = n;
}

/// A `Debug`-only value the dep observes through the `Debug` fallback rung.
#[derive(Debug)]
struct Report {
    hits: u32,
}

fn make_report() -> Report {
    Report { hits: 3 }
}

/// A fallible dep whose `Ok` payload is `Debug` but NOT `ToValue`, exercising
/// the `Debug`-floor `Result` rung through the real macro expansion.
fn try_report(ok: bool) -> Result<Report, String> {
    if ok {
        Ok(Report { hits: 4 })
    } else {
        Err("no report".to_string())
    }
}

const CONFIG: i64 = 99;

#[watch_operation(component = "annotations")]
fn use_opt(n: i64) -> i64 {
    let r = watch_dep!("opt", opt_dep(n));
    r.unwrap_or(-1)
}

#[watch_operation(component = "annotations")]
fn use_plain(n: i64) -> i64 {
    watch_dep!("double", double(n))
}

#[watch_operation(component = "annotations")]
fn use_unit(n: i64) -> i64 {
    watch_dep!("record", record(n));
    n
}

#[watch_operation(component = "annotations")]
fn use_report() -> i64 {
    let r = watch_dep!("report", make_report());
    i64::from(r.hits)
}

#[watch_operation(component = "annotations")]
fn use_config() -> i64 {
    watch_dep!("cfg", CONFIG)
}

#[watch_operation(component = "annotations")]
fn use_try_report(ok: bool) -> i64 {
    match watch_dep!("try_report", try_report(ok)) {
        Ok(r) => i64::from(r.hits),
        Err(_) => -1,
    }
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn option_dep_some_is_result_and_none_is_empty() {
    reset();
    assert_eq!(use_opt(5), 5);
    assert_eq!(take_spans()[1].events, vec![result(5)]);

    reset();
    assert_eq!(use_opt(-1), -1);
    assert_eq!(take_spans()[1].events, vec![empty()]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn plain_value_dep_is_a_result() {
    reset();
    assert_eq!(use_plain(4), 8);
    let spans = take_spans();
    assert_eq!(spans[1].events, vec![result(8)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn unit_dep_is_a_result_via_the_debug_ladder() {
    reset();
    assert_eq!(use_unit(7), 7);
    let spans = take_spans();
    // `()` is neither `ToValue` nor `Display`; it falls to the `Debug` rung.
    assert_eq!(spans[1].events, vec![result(Value::String("()".into()))]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn structural_non_to_value_dep_falls_back_to_debug() {
    reset();
    assert_eq!(use_report(), 3);
    let spans = take_spans();
    assert_eq!(
        spans[1].events,
        vec![result(Value::String("Report { hits: 3 }".into()))]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn non_call_dep_has_zero_inputs_and_is_value_only() {
    reset();
    assert_eq!(use_config(), 99);
    let spans = take_spans();
    let child = &spans[1];
    // A bare path captures no inputs.
    assert_eq!(child.attributes, op_attrs("annotations", "cfg", &[]));
    assert_eq!(child.events, vec![result(99)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn non_to_value_result_dep_keeps_ok_err_disposition() {
    // `Ok(Report)`: `Report` is `Debug` but not `ToValue`, so the child span
    // records a `result` with the `Debug` string — disposition is NOT lost.
    reset();
    assert_eq!(use_try_report(true), 4);
    let spans = take_spans();
    assert_eq!(
        spans[1].events,
        vec![result(Value::String("Report { hits: 4 }".into()))]
    );

    // `Err`: recorded as an `error` (fallback name), never mis-recorded as a
    // success even though the `Ok` payload is not `ToValue`.
    reset();
    assert_eq!(use_try_report(false), -1);
    let spans = take_spans();
    assert_eq!(spans[1].events, vec![error("error", "no report")]);
}
