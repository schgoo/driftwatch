//! The `aliases` fixture: an operation whose return type is a **foreign type
//! alias** that hides its error channel from the current string pipeline.
//!
//! [`first_byte`] returns `std::io::Result<T>`. The alias collapses
//! `Result<T, E>` down to a single visible generic argument, so the build-time
//! string classifier in `extract::classify_return` sees only the `T` and emits
//! **no** error outcome — the registry validates cleanly today precisely
//! *because* the alias hides the error.
//!
//! # Resolver-sensitive entries
//!
//! This is the Tier-1 case the upcoming rust-analyzer static resolver (roadmap
//! PR-4) will change. When it lands, real type inference recovers the aliased
//! error, so re-blessing `tests/golden/registry.json` will grow the
//! `outcomes.errors[]` array on [`first_byte`] with the name-only foreign
//! `io::Error` (its last path segment, `Error`). That diff is the acceptance
//! evidence for the resolver; nothing else in the golden moves.

use std::io;

use annotations::watch_operation;

/// Returns the first byte of `text` as an integer, erroring on empty input.
///
/// The `io::Result<i64>` return type is an alias for `Result<i64, io::Error>`;
/// today the string pipeline sees only the `i64` and emits no error outcome.
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
