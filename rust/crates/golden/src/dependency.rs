//! The `dependency` fixture: an annotated operation with a real `watch_dep!`
//! nested call.
//!
//! A `watch_dep!` call emits a child operation span linked to the parent, with
//! component inheritance (the dep declares no override, so it inherits
//! `golden.parsing`) and dep inputs keyed by identifier (`text`) and position
//! (`arg1` for the `16` literal). A good parse records a `conformance.result` on
//! the child dep; a bad parse records a `conformance.error` with Error status,
//! its `Err` recovered by `unwrap_or`.

use annotations::{watch_dep, watch_operation};

/// Parses `text` as a hexadecimal integer through an observed dependency call
/// (`i64::from_str_radix`), falling back to `-1` on a parse error.
#[must_use]
#[watch_operation(component = "golden.parsing")]
pub fn to_int(text: &str) -> i64 {
    let parsed = watch_dep!("parse", i64::from_str_radix(text, 16));
    parsed.unwrap_or(-1)
}
