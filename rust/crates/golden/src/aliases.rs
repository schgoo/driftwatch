//! The `aliases` fixture: an operation whose return type is a **foreign type
//! alias** that hides its error channel from a naive string classifier.
//!
//! [`first_byte`] returns `std::io::Result<T>`. The alias collapses
//! `Result<T, E>` down to a single visible generic argument, so a build-time
//! string classifier sees only the `T` and would emit **no** error outcome. The
//! static resolver, using real type inference, sees through the alias and
//! recovers the name-only foreign `io::Error` (its last path segment, `Error`).
//!
//! # Resolver-recovered entry
//!
//! This is the Tier-1 case the rust-analyzer static resolver recovers: the
//! resolved `tests/golden/registry.json` carries `first_byte`'s
//! `outcomes.errors[]` with the name-only foreign `io::Error` (`Error`), a
//! channel a string pipeline drops.

use std::io;

use annotations::watch_operation;

/// Returns the first byte of `text` as an integer, erroring on empty input.
///
/// The `io::Result<i64>` return type is an alias for `Result<i64, io::Error>`;
/// the resolver sees through the alias and recovers the name-only `io::Error`.
///
/// # Errors
///
/// Returns an [`io::Error`] of kind [`io::ErrorKind::UnexpectedEof`] when
/// `text` is empty.
#[watch_operation(component = "golden.aliases")]
pub fn first_byte(text: &str) -> io::Result<i64> {
    match text.bytes().next() {
        Some(byte) => Ok(i64::from(byte)),
        None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input")),
    }
}
