//! Operations covering the four CTSC completion dispositions:
//!
//! - [`add`] → a `conformance.result` (scalar success);
//! - [`first_even`] → a `conformance.empty` (`Option::None`, OK status);
//! - [`log_only`] → no completion event (unit `()`, OK status);
//! - [`lookup`] → a `conformance.error` (a `#[derive(Watchable)]` variant name
//!   and payload, Error status).

use annotations::{Watchable, watch_operation};

/// A declared error whose `#[derive(Watchable)]` decomposition drives the
/// `conformance.error.name` (the variant tag) and `conformance.error.value`
/// (the field map).
#[derive(Watchable, Debug, Clone, Copy)]
pub enum LookupError {
    /// The lookup did not complete within the allotted time.
    Timeout {
        /// The elapsed budget, in seconds, before the lookup was abandoned.
        seconds: i64,
    },
}

/// Adds two integers, yielding a direct scalar `conformance.result`.
#[must_use]
#[watch_operation(component = "golden.outcomes")]
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Returns the first even element, or `None` — the `None` path is a
/// `conformance.empty` (deliberate absence) with OK status.
#[must_use]
#[watch_operation(component = "golden.outcomes")]
pub fn first_even(xs: Vec<i64>) -> Option<i64> {
    xs.into_iter().find(|n| n % 2 == 0)
}

/// A unit-returning operation: `()` records no completion event and stays OK.
#[watch_operation(component = "golden.outcomes")]
pub fn log_only(msg: &str) {
    let _ = msg;
}

/// Fails for `id == 0` with a declared [`LookupError`] (a `conformance.error`
/// with Error status); otherwise echoes the id as a result.
///
/// # Errors
///
/// Returns [`LookupError::Timeout`] when `id == 0`.
#[watch_operation(component = "golden.outcomes")]
pub fn lookup(id: i64) -> Result<i64, LookupError> {
    if id == 0 {
        return Err(LookupError::Timeout { seconds: 30 });
    }
    Ok(id)
}
