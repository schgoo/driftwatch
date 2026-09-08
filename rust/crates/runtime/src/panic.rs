//! Panic capture for the panic-disposition path (`conformance.fault`).
//!
//! An operation that panics must still emit a fault event and then let the panic
//! propagate. The synchronous path uses [`std::panic::catch_unwind`] directly in
//! macro-generated code; the asynchronous path needs a poll-boundary equivalent,
//! because a `Future`'s body runs incrementally across many `poll` calls and a
//! panic can surface on any of them. [`catch_unwind_fut`] provides that: it wraps
//! each `poll` in `catch_unwind`, converting a panic into an `Err` outcome the
//! caller can turn into a fault before resuming the unwind.
//!
//! This is dependency-free (no async runtime, no `unsafe`): it drives the future
//! with [`std::future::poll_fn`] and pins it on the heap so the returned future
//! stays `Unpin`.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::Poll;

/// Poll `f` to completion, catching a panic on any `poll` and returning it as
/// `Err`.
///
/// `Ok(output)` is the future's normal value; `Err(payload)` is the panic
/// payload from whichever `poll` unwound, ready to hand to
/// [`std::panic::resume_unwind`] after recording the fault. `AssertUnwindSafe`
/// is required because a partially-polled future is not `UnwindSafe`; the caller
/// resumes the unwind immediately, so no observation is made of a
/// possibly-inconsistent state through this boundary.
///
/// # Errors
///
/// Returns `Err(Box<dyn Any + Send>)` — the caught panic payload — if `f` panics
/// on any `poll`. Otherwise returns `Ok(F::Output)`.
pub async fn catch_unwind_fut<F: Future>(f: F) -> Result<F::Output, Box<dyn Any + Send>> {
    let mut f = Box::pin(f);
    std::future::poll_fn(
        move |cx| match catch_unwind(AssertUnwindSafe(|| f.as_mut().poll(cx))) {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal executor: poll to completion with a noop waker. The futures
    /// under test are always immediately ready, so no real wakeup is needed.
    fn block_on<F: Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[test]
    fn sync_catch_unwind_reports_a_panic_as_err() {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = catch_unwind(AssertUnwindSafe(|| -> i64 { panic!("sync boom") }));
        std::panic::set_hook(prev);
        // `expect_err` (not `assert!(is_err())`) satisfies `assertions_on_result_states`.
        drop(outcome.expect_err("a sync panic must surface as Err"));
    }

    #[test]
    fn async_catch_unwind_fut_reports_a_panic_as_err() {
        #[expect(
            clippy::unused_async,
            reason = "an async future that panics on first poll; awaited via catch_unwind_fut"
        )]
        async fn boom() -> i64 {
            panic!("async boom")
        }
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = block_on(catch_unwind_fut(boom()));
        std::panic::set_hook(prev);
        drop(outcome.expect_err("an async panic must surface as Err"));
    }

    #[test]
    fn async_catch_unwind_fut_passes_through_a_normal_value() {
        let outcome = block_on(catch_unwind_fut(async { 7_i64 }));
        assert_eq!(outcome.ok(), Some(7));
    }
}
