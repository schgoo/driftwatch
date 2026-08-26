//! The user-facing annotation surface for Driftwatch.
//!
//! A crate depending only on this facade can annotate its operations, state, and
//! inline checkpoints; running the annotated code emits a [`WatchEvent`] stream
//! into the runtime's thread-local buffer (drained via [`take_events`]), and the
//! annotated operations and types appear in the link-time discovery registry.
//! Annotated code never names the `runtime` or `linkme` crates directly — the
//! macros funnel every reference through the hidden [`__rt`] re-export, so the
//! facade is the single dependency users add.
//!
//! # Public surface
//!
//! - Macros: [`watch_operation`], [`watch_component`], [`watch_point`], and
//!   [`watch_input`].
//! - Runtime items users interact with directly: [`Value`], [`ToValue`],
//!   [`WatchEvent`], [`take_events`], and [`reset`].
//!
//! # Production gating
//!
//! The `trace` feature (on by default) forwards to `annotations-macros/trace`.
//! Building with `--no-default-features` expands every macro to identity: the
//! annotated items compile unchanged and emit nothing, with no registry statics
//! and no references to the runtime.

pub use annotations_macros::{watch_component, watch_input, watch_operation, watch_point};
pub use runtime::{ToValue, Value, WatchEvent, reset, take_events};

/// Hidden plumbing the generated macro code funnels through. Not part of the
/// stable public API — user code should never name it directly.
#[doc(hidden)]
pub mod __rt {
    pub use runtime::linkme;
    pub use runtime::*;
}
