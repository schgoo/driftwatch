//! The `faults` fixtures: panic-disposition operations.
//!
//! - [`boom`] panics directly, recording a target `conformance.fault`
//!   (`type = "unexpected"`, `observer = "target"`), Error status, and no
//!   completion event;
//! - [`cascade_outer`] calls a panicking dependency, cascading the fault to
//!   **both** the enclosing operation and its dependency span, each Error with
//!   no completion.

use annotations::{watch_dep, watch_operation};

/// Panics on negative input, exercising the target-fault disposition; the panic
/// re-propagates so callers see unchanged behavior.
///
/// # Panics
///
/// Panics when `x` is negative — the fixture's target-fault path.
#[must_use]
#[watch_operation(component = "golden.faults")]
pub fn boom(x: i64) -> i64 {
    assert!(x >= 0, "negative input");
    x
}

/// An unannotated dependency that panics on bad input; the fixture drives that
/// panic path via `n < 0` to cascade a fault through the enclosing `watch_dep!`.
fn cascade_inner(n: i64) -> i64 {
    assert!(n >= 0, "bad n");
    n * 2
}

/// Calls the panicking [`cascade_inner`] dependency; the panic faults both this
/// operation and the enclosing dependency span.
#[must_use]
#[watch_operation(component = "golden.faults")]
pub fn cascade_outer(n: i64) -> i64 {
    let inner = watch_dep!("cascade_inner", cascade_inner(n));
    inner + 1
}
