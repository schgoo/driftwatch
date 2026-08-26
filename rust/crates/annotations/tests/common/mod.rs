//! Shared helpers for the annotation integration tests.
//!
//! `ev`/`run` build the two [`WatchEvent`] shapes the emission assertions
//! compare against. `ev` takes `impl ToValue`, so tests pass raw values and let
//! the conversion do the wrapping: integers (`ev("add.a", 2)`), strings
//! (`ev("greet.subject", "ada")`), and collections (`ev("xs", vec![1, 2])`) all
//! work directly, as do pre-built [`Value`]s (`Value: ToValue` is the identity).
//!
//! Not every test file uses every helper, so this module allows dead code.
#![allow(
    dead_code,
    reason = "shared across test binaries; not every binary uses every helper"
)]

use annotations::{ToValue, WatchEvent};

/// A named [`WatchEvent::Event`] carrying `value`'s [`ToValue`] encoding.
pub fn ev(name: &str, value: impl ToValue) -> WatchEvent {
    WatchEvent::Event {
        name: name.to_string(),
        value: value.to_value(),
    }
}

/// A [`WatchEvent::Run`] marker for the operation `name`.
pub fn run(name: &str) -> WatchEvent {
    WatchEvent::Run {
        operation: name.to_string(),
    }
}
