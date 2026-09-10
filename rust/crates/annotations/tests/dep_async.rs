//! `watch_dep!("fetch", fetch(id).await)?`: async dependency observation.
//!
//! A trailing `.await` lives *inside* the macro (peeled for input-capture and
//! re-applied inside the `catch_unwind_fut` path); `?` composes *outside*.
//!
//! Coverage:
//! - `Ok`: child `conformance.result`, then the outside `?` propagates and the
//!   op records its own `result`;
//! - `Err`: child `conformance.error` (fallback name), then `?` propagates and
//!   the op records a structural `error`;
//! - panic: the ratified Option A cascade — a `conformance.fault` on the dep
//!   span and the op span, both status `Error`, no completion.

mod common;

use annotations::{SpanName, SpanStatus, Value, reset, take_spans, watch_dep, watch_operation};
use common::{error, fault, op_attrs, result};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// An unannotated async dependency with a real suspension point.
async fn fetch(id: i64) -> Result<i64, String> {
    std::future::ready(()).await;
    assert!(id >= 0, "bad id");
    if id == 0 {
        return Err("not found".to_string());
    }
    Ok(id * 10)
}

#[watch_operation(component = "annotations")]
async fn load(id: i64) -> Result<i64, String> {
    let v = watch_dep!("fetch", fetch(id).await)?;
    Ok(v + 1)
}

/// A minimal executor: the futures under test suspend once then complete, so a
/// noop-waker poll loop suffices.
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

/// Run `f`, swallowing the default panic hook's stderr noise, and assert it
/// unwound (proving the fault path re-propagates rather than swallows).
fn expect_panic(f: impl FnOnce()) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    outcome.expect_err("the panic must propagate, not be swallowed");
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_dep_ok_records_result_on_both_spans() {
    reset();
    assert_eq!(block_on(load(3)), Ok(31));
    let spans = take_spans();
    assert_eq!(spans.len(), 2);
    let (parent, child) = (&spans[0], &spans[1]);
    assert_eq!(child.parent_span_id, Some(parent.span_id));
    // The peeled `.await` argument is captured as the dep's input.
    assert_eq!(
        child.attributes,
        op_attrs("annotations", "fetch", &[("id", Value::Integer(3))])
    );
    assert_eq!(child.events, vec![result(30)]);
    assert_eq!(parent.events, vec![result(31)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_dep_err_records_child_error_then_op_error() {
    reset();
    block_on(load(0)).unwrap_err();
    let spans = take_spans();
    let (parent, child) = (&spans[0], &spans[1]);
    assert_eq!(child.events, vec![error("error", "not found")]);
    assert_eq!(parent.events, vec![error("String", "not found")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_dep_panic_cascades_a_fault_to_every_enclosing_frame() {
    reset();
    expect_panic(|| {
        let _ = block_on(load(-1));
    });
    let spans = take_spans();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(spans[1].name, SpanName::Operation);

    // Outer operation frame: faulted, no completion.
    assert_eq!(spans[0].status, SpanStatus::Error);
    assert_eq!(spans[0].events, vec![fault("target", "bad id")]);

    // Inner dep frame: also faulted, nested under the outer, no completion.
    assert_eq!(spans[1].status, SpanStatus::Error);
    assert_eq!(spans[1].parent_span_id, Some(spans[0].span_id));
    assert_eq!(
        spans[1].attributes,
        op_attrs("annotations", "fetch", &[("id", Value::Integer(-1))])
    );
    assert_eq!(spans[1].events, vec![fault("target", "bad id")]);
}
