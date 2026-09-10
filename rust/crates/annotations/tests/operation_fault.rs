//! `#[watch_operation]` panic disposition as a CTSC `conformance.fault`.
//!
//! Feature-matrix coverage: an operation that panics records a partial trace —
//! the operation span with its inputs, status `Error`, and exactly one
//! `conformance.fault` event (`observer = "target"`, `message` = the raw panic
//! payload) — and NO `result`/`empty`/`error` completion event. The panic still
//! propagates (`resume_unwind`), so callers see unchanged behavior. Both the
//! synchronous and the asynchronous wrapper are exercised.

mod common;

use annotations::{
    SpanName, SpanStatus, Value, reset, take_spans, watch_dep, watch_operation, watch_point,
};
use common::{fault, obs, op_attrs};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[watch_operation(component = "annotations")]
fn boom_sync(x: i64) -> i64 {
    assert!(x >= 0, "negative input");
    x
}

#[watch_operation(component = "annotations")]
async fn boom_async(x: i64) -> i64 {
    // Await point so the panic is caught across a suspension.
    std::future::ready(()).await;
    assert!(x >= 0, "negative input");
    x
}

/// A minimal executor mirroring `operation_returns.rs`: the operations under
/// test are immediately ready, so a noop-waker poll loop suffices.
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

/// Run `f`, swallowing the default panic hook's stderr noise, and assert the
/// call unwound (proving the fault path re-propagates rather than swallows).
fn expect_panic(f: impl FnOnce()) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    // `expect_err` (not `assert!(is_err())`) satisfies `assertions_on_result_states`.
    outcome.expect_err("the panic must propagate, not be swallowed");
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn sync_panic_emits_a_fault_sets_error_and_propagates() {
    reset();
    expect_panic(|| {
        let _ = boom_sync(-1);
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SpanName::Operation);
    // The span records the failure status.
    assert_eq!(spans[0].status, SpanStatus::Error);
    // Partial trace: the operation inputs are still present.
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "boom_sync", &[("x", Value::Integer(-1))])
    );
    // Exactly one fault event; no result/empty/error completion.
    assert_eq!(spans[0].events, vec![fault("target", "negative input")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_panic_emits_a_fault_sets_error_and_propagates() {
    reset();
    expect_panic(|| {
        let _ = block_on(boom_async(-1));
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "boom_async", &[("x", Value::Integer(-1))])
    );
    assert_eq!(spans[0].events, vec![fault("target", "negative input")]);
}

// ---------------------------------------------------------------------------
// Downcast-ladder and partial-trace coverage (message arms + prior events).
// ---------------------------------------------------------------------------

/// Emits an observation, then panics: the observation must survive on the
/// partial trace, followed by the fault.
#[watch_operation(component = "annotations")]
fn observe_then_panic(x: i64) -> i64 {
    watch_point!("seen", &x);
    assert!(x >= 0, "negative input");
    x
}

/// `panic!` with a format argument yields a `String` payload (the `String` arm
/// of the downcast ladder).
#[watch_operation(component = "annotations")]
fn panic_with_string(x: i64) -> i64 {
    panic!("bad value: {x}")
}

/// `panic_any` with an `i64` payload is neither `&str` nor `String`, exercising
/// the `"<non-string panic payload>"` fallback arm.
#[watch_operation(component = "annotations")]
fn panic_nonstring(x: i64) -> i64 {
    std::panic::panic_any(x)
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn observation_before_panic_is_retained_then_the_fault_follows() {
    reset();
    expect_panic(|| {
        let _ = observe_then_panic(-1);
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "observe_then_panic",
            &[("x", Value::Integer(-1))]
        )
    );
    // The observation emitted before the panic is retained, then the fault
    // follows it — and there is NO completion event.
    assert_eq!(
        spans[0].events,
        vec![obs("seen", -1), fault("target", "negative input")]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn string_panic_payload_is_captured_verbatim() {
    reset();
    expect_panic(|| {
        let _ = panic_with_string(-1);
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "panic_with_string",
            &[("x", Value::Integer(-1))]
        )
    );
    assert_eq!(spans[0].events, vec![fault("target", "bad value: -1")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn non_string_panic_payload_falls_back() {
    reset();
    expect_panic(|| {
        let _ = panic_nonstring(-1);
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "panic_nonstring",
            &[("x", Value::Integer(-1))]
        )
    );
    assert_eq!(
        spans[0].events,
        vec![fault("target", "<non-string panic payload>")]
    );
}

// ---------------------------------------------------------------------------
// Nested-dep panic cascade — ratified Option A.
// ---------------------------------------------------------------------------

/// An unannotated dependency that panics on bad input. Returns `Result` because
/// `watch_dep!` observes the call's `Ok`/`Err` disposition; the `Err` branch
/// keeps the wrapper honest (the test drives the panic path via `n < 0`).
fn cascade_inner(n: i64) -> Result<i64, String> {
    assert!(n >= 0, "bad n");
    if n > 1_000 {
        return Err("out of range".to_string());
    }
    Ok(n * 2)
}

#[watch_operation(component = "annotations")]
fn cascade_outer(n: i64) -> Result<i64, String> {
    let inner = watch_dep!("cascade_inner", cascade_inner(n))?;
    Ok(inner + 1)
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn nested_dep_panic_cascades_a_fault_to_every_enclosing_frame() {
    // Pins ratified Option A: a panic faults every enclosing watched frame
    // (the operation AND each enclosing dep), each with status Error and no
    // completion event.
    reset();
    expect_panic(|| {
        let _ = cascade_outer(-1);
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(spans[1].name, SpanName::Operation);

    // Outer operation frame: faulted, inputs retained, no completion.
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "cascade_outer", &[("n", Value::Integer(-1))])
    );
    assert_eq!(spans[0].events, vec![fault("target", "bad n")]);

    // Inner dep frame: also faulted, nested under the outer, no completion. Its
    // input key is the bare argument identifier `n`; it inherits the parent
    // component `annotations`.
    assert_eq!(spans[1].status, SpanStatus::Error);
    assert_eq!(spans[1].parent_span_id, Some(spans[0].span_id));
    assert_eq!(
        spans[1].attributes,
        op_attrs("annotations", "cascade_inner", &[("n", Value::Integer(-1))])
    );
    assert_eq!(spans[1].events, vec![fault("target", "bad n")]);
}
