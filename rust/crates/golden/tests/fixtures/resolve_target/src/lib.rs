//! A tiny fixture crate the resolver-emit path aims rust-analyzer at.
//!
//! Its headline value is a single aliased operation: `first_byte` returns a
//! `std::io::Result<i64>` alias, which hides the `io::Error` channel from the
//! build-time string pipeline (that path reports the op as infallible). Real
//! type inference recovers the error as the name-only `Error` (trace-contract
//! producer-choice #4), so the resolver-emitted `registry.json` carries an error
//! outcome the string-derived one drops.
//!
//! The `#[watch_operation]` marker is left as an inert (unresolved) attribute on
//! purpose: the resolver selects items by scanning source-attribute text and
//! resolves the underlying signature through rust-analyzer with proc-macro
//! expansion disabled, so the fixture needs no dependency on the annotation
//! macros.

use std::io;

/// A local alias hiding `Result<T, io::Error>` behind a single visible generic.
pub type IoAlias<T> = io::Result<T>;

/// Returns the first byte of `text` through a `std::io::Result` alias.
///
/// The alias hides the `io::Error` channel from the string pipeline; real type
/// inference recovers it as the name-only `Error` (producer-choice #4).
#[watch_operation(component = "resolve.demo")]
pub fn first_byte(text: &str) -> IoAlias<i64> {
    match text.bytes().next() {
        Some(byte) => Ok(i64::from(byte)),
        None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input")),
    }
}
