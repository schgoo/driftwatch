//! A tiny fixture crate exercising the resolver's headline value: recovering
//! error channels that a `type` alias / foreign `Result` alias hides from the
//! build-time string pipeline, plus `#[derive(Watchable)]` enum decomposition.
//!
//! The `#[watch_operation]` / `#[derive(Watchable)]` markers are left as
//! unresolved (inert) attributes on purpose: the resolver selects items by
//! scanning source-attribute text and resolves the *underlying* signature /
//! definition through rust-analyzer with proc-macro expansion disabled, so the
//! fixture needs no dependency on the annotation macros.

use std::io;

/// A local alias hiding `Result<T, io::Error>` behind a single visible generic.
pub type IoAlias<T> = io::Result<T>;

/// Returns the first byte of `text` through a `std::io::Result` alias.
///
/// The alias hides the `io::Error` channel from the string pipeline; real type
/// inference recovers it as the name-only `Error` (producer-choice #4).
#[watch_operation(component = "aliased")]
pub fn first_byte(text: &str) -> IoAlias<i64> {
    match text.bytes().next() {
        Some(byte) => Ok(i64::from(byte)),
        None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input")),
    }
}

/// Parses `s` as an integer through `anyhow::Result` — a second, distinct
/// foreign error channel whose `anyhow::Error` also collapses to `Error`.
#[watch_operation(component = "aliased")]
pub fn parse_amount(s: &str) -> anyhow::Result<i64> {
    Ok(s.trim().parse()?)
}

/// A `Watchable` error enum the resolver decomposes into a tagged union.
#[derive(Watchable, Debug)]
pub enum ChargeError {
    /// A declined charge, carrying the offending amount in cents.
    Declined {
        /// The rejected amount, in cents.
        code: i64,
    },
    /// An overflowing charge (unit variant — no payload).
    Overflow,
}
