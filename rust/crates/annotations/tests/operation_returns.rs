//! `#[watch_operation]` completion dispositions as CTSC span events.
//!
//! Feature-matrix coverage: the completion mapping across return kinds.
//! - unit / `()` → NO completion event;
//! - scalar → one `conformance.result` (unwrapped);
//! - `Result<T, E>` → `result` (Ok) / structural `error` (Err);
//! - `Result<Option<T>, E>` → `result` (Ok(Some)) / `empty` (Ok(None)) / `error`;
//! - `Option<T>` → `result` (Some) / `empty` (None);
//! - a `#[derive(Watchable)]` error enum → per-variant `error.name` + payload;
//! - `async fn` → completion emitted after the awaited body completes.

mod common;

use annotations::{SpanName, Value, Watchable, reset, take_spans, watch_operation};
use common::{empty, error, op_attrs, result};
use std::collections::BTreeMap;

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::needless_pass_by_value,
        reason = "identity (trace-off) form borrows `msg`; the shape exercises a unit-return op with a value param"
    )
)]
#[watch_operation(component = "annotations")]
fn log_only(msg: String) {
    let _ = msg;
}

#[watch_operation(component = "annotations")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[watch_operation(component = "annotations")]
fn checked_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        return Err("divide by zero".to_string());
    }
    Ok(a / b)
}

#[watch_operation(component = "annotations")]
fn first_even(xs: Vec<i64>) -> Option<i64> {
    xs.into_iter().find(|n| n % 2 == 0)
}

#[watch_operation(component = "annotations")]
fn maybe_positive(n: i64) -> Result<Option<i64>, String> {
    if n < 0 {
        return Err("negative".to_string());
    }
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(n))
}

/// A `#[derive(Watchable)]` error enum: its `ToValue` decomposition drives the
/// per-variant `conformance.error.name` (variant tag) + payload value.
#[cfg_attr(
    not(feature = "trace"),
    allow(dead_code, reason = "constructed only on-trace")
)]
#[derive(Watchable, Debug, PartialEq)]
enum ApiError {
    NotFound,
    Timeout { seconds: i64 },
}

#[watch_operation(component = "annotations")]
fn lookup(id: i64) -> Result<i64, ApiError> {
    if id == 0 {
        return Err(ApiError::NotFound);
    }
    if id < 0 {
        return Err(ApiError::Timeout { seconds: 30 });
    }
    Ok(id)
}

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::unused_async,
        reason = "identity (trace-off) form has no await; the traced form wraps the body in an awaited async block"
    )
)]
#[watch_operation(component = "annotations")]
async fn doubled(id: i64) -> i64 {
    id * 2
}

/// A minimal executor: the operations under test are immediately ready, so a
/// noop-waker poll loop suffices without pulling in an async runtime.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    loop {
        if let std::task::Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn unit_return_emits_no_completion_event() {
    reset();
    log_only("hello".to_string());
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "log_only",
            &[("msg", Value::String("hello".into()))]
        )
    );
    assert_eq!(spans[0].events, vec![]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn scalar_return_is_a_direct_result() {
    reset();
    assert_eq!(add(2, 3), 5);
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "add",
            &[("a", Value::Integer(2)), ("b", Value::Integer(3))]
        )
    );
    assert_eq!(spans[0].events, vec![result(5)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn result_ok_is_a_result_event() {
    reset();
    assert_eq!(checked_div(10, 2), Ok(5));
    let spans = take_spans();
    assert_eq!(spans[0].events, vec![result(5)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn result_err_decomposes_to_error_event() {
    reset();
    assert_eq!(checked_div(1, 0), Err("divide by zero".to_string()));
    let spans = take_spans();
    // name = fallback (last segment of E = `String`); value = the Display string.
    assert_eq!(spans[0].events, vec![error("String", "divide by zero")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn option_some_is_a_result_event() {
    reset();
    assert_eq!(first_even(vec![1, 4, 6]), Some(4));
    let spans = take_spans();
    assert_eq!(spans[0].events, vec![result(4)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn option_none_is_an_empty_event() {
    reset();
    assert_eq!(first_even(vec![1, 3]), None);
    let spans = take_spans();
    assert_eq!(spans[0].events, vec![empty()]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn result_option_peels_ok_some_ok_none_and_err() {
    reset();
    assert_eq!(maybe_positive(5), Ok(Some(5)));
    assert_eq!(take_spans()[0].events, vec![result(5)]);

    reset();
    assert_eq!(maybe_positive(0), Ok(None));
    assert_eq!(take_spans()[0].events, vec![empty()]);

    reset();
    assert_eq!(maybe_positive(-1), Err("negative".to_string()));
    assert_eq!(take_spans()[0].events, vec![error("String", "negative")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn watchable_error_enum_carries_variant_name_and_payload() {
    reset();
    assert_eq!(lookup(0), Err(ApiError::NotFound));
    // Unit variant: name = tag, value = CTSC unit (empty map).
    assert_eq!(
        take_spans()[0].events,
        vec![error("NotFound", Value::Map(BTreeMap::new()))]
    );

    reset();
    assert_eq!(lookup(-1), Err(ApiError::Timeout { seconds: 30 }));
    // Named variant: name = tag, value = the field map.
    assert_eq!(
        take_spans()[0].events,
        vec![error(
            "Timeout",
            Value::Map(BTreeMap::from([(
                "seconds".to_string(),
                Value::Integer(30)
            )]))
        )]
    );

    reset();
    assert_eq!(lookup(7), Ok(7));
    assert_eq!(take_spans()[0].events, vec![result(7)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_completion_is_emitted_after_the_body_completes() {
    reset();
    assert_eq!(block_on(doubled(21)), 42);
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "doubled", &[("id", Value::Integer(21))])
    );
    assert_eq!(spans[0].events, vec![result(42)]);
}
