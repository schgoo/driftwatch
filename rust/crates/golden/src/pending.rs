//! The `pending` fixture: operations whose alias-hidden channels only real type
//! inference recovers — the cases the rust-analyzer static resolver resolves.
//!
//! These four operations (plus the owned [`ChargeError`] enum) were the Tier-2
//! resolver-oracle fixtures; with the resolver now the default registry-emit
//! path they fold into the Tier-1 corpus. Each exercises a channel the old
//! build-time *string* pipeline could not see through:
//!
//! - [`charge`] → `ChargeResult<i64>` — the **semantic** case. The local
//!   `ChargeResult` alias hides the `Result` shape; real inference recovers
//!   `result: i64` + a name-only `ChargeError` error (named after the owned
//!   [`ChargeError`] type's last path segment, per producer-choice #4).
//! - [`first_byte`] → `io::Result<i64>` — the **structural / cross-crate** case.
//!   The foreign alias shows one visible generic arg, so the string pipeline
//!   reported it infallible; the resolver recovers the name-only foreign
//!   `Error` (`io::Error`'s last path segment). It carries no `types[]` entry.
//! - [`parse_amount`] → `anyhow::Result<i64>` — the **foreign-error collision**
//!   case. A second, distinct foreign crate whose `anyhow::Error` also collapses
//!   to the *same* name-only `Error` — the collision producer-choice #4
//!   sanctions; the resolver must not disambiguate it.
//! - [`elapsed`] → `Millis` — the **alias→primitive** baseline. The local alias
//!   normalizes to the primitive `u64`.
//!
//! The error `ty` field stays `None` throughout: v1 declares errors by name
//! only, and foreign-error name collisions are spec-sanctioned. `ChargeError`'s
//! `#[derive(Watchable)]` stamps its component with the crate's `CARGO_PKG_NAME`
//! (`golden`), *not* the `resolver.pending` operation tag, so it registers under
//! the crate-name `golden` component while the operations sit in
//! `resolver.pending` — two components, by design.

use std::io;

use annotations::{Watchable, watch_operation};

/// An owned `Watchable` error enum: the **semantic** recovery case.
///
/// Because we own this type and derive [`Watchable`], its component is the
/// crate's `CARGO_PKG_NAME` (`golden`) and the resolver names [`charge`]'s
/// recovered error after this type's last path segment — `ChargeError`.
#[derive(Watchable, Debug, Clone, Copy)]
pub enum ChargeError {
    /// A declined charge, carrying the offending amount.
    Declined {
        /// The rejected amount, in cents.
        code: i64,
    },
}

/// A local alias for a fallible charge result: `Result<T, ChargeError>`.
type ChargeResult<T> = core::result::Result<T, ChargeError>;

/// Charges `cents`, declining a negative amount — the **semantic** (owned,
/// `Watchable`) error-recovery case.
///
/// The `ChargeResult` alias hides the `Result` shape; real inference normalizes
/// it to `Result<i64, ChargeError>`: `result: i64` + a recovered `ChargeError`.
///
/// # Errors
///
/// Returns [`ChargeError::Declined`] when `cents` is negative.
#[watch_operation(component = "resolver.pending")]
pub fn charge(cents: i64) -> ChargeResult<i64> {
    if cents < 0 {
        Err(ChargeError::Declined { code: cents })
    } else {
        Ok(cents)
    }
}

/// Returns the first byte of `text` as an integer — the **structural /
/// cross-crate** error-recovery case.
///
/// `io::Result<i64>` is a foreign alias for `Result<i64, io::Error>`. Real
/// inference recovers the error; because `io::Error` is foreign (never
/// `Watchable`), the recovered outcome is name-only — its last path segment
/// `Error` (producer-choice #4), with no `types[]` entry.
///
/// # Errors
///
/// Returns an [`io::Error`] of kind [`io::ErrorKind::UnexpectedEof`] when
/// `text` is empty.
#[watch_operation(component = "resolver.pending")]
pub fn first_byte(text: &str) -> io::Result<i64> {
    match text.bytes().next() {
        Some(byte) => Ok(i64::from(byte)),
        None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input")),
    }
}

/// Parses `s` as an integer through `anyhow::Result` — the **foreign-error
/// collision** case, a second, distinct foreign crate.
///
/// `anyhow::Result<i64>` is a foreign alias for `Result<i64, anyhow::Error>`.
/// `anyhow::Error` is foreign (never `Watchable`), so its recovered outcome is
/// the name-only last path segment `Error` — the *same* name [`first_byte`]
/// gets from the unrelated `io::Error`. The resolver must NOT disambiguate the
/// collision.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] when `s` does not parse as an `i64`.
#[watch_operation(component = "resolver.pending")]
pub fn parse_amount(s: &str) -> anyhow::Result<i64> {
    Ok(s.trim().parse()?)
}

/// An undeclared local type alias: `Millis` is not a `Watchable` type.
type Millis = u64;

/// Returns the elapsed tick count as a `Millis` alias — the **alias→primitive**
/// case; the resolver normalizes it to the primitive `u64`.
#[must_use]
#[watch_operation(component = "resolver.pending")]
pub fn elapsed(ticks: u64) -> Millis {
    ticks
}
