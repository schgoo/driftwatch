//! Shared helpers for the annotation integration tests.
//!
//! `ev`/`run` build the two [`WatchEvent`] shapes the emission assertions
//! compare against. `ev` takes `impl ToValue`, so tests can pass raw scalars
//! (`ev("add.a", 2_i64)`) or pre-built [`Value`]s (`Value: ToValue` is the
//! identity) interchangeably; for string params call `.to_string()` first,
//! since there is no `impl ToValue for str`.
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
