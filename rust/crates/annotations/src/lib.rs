//! The extraction-time annotation surface for Driftwatch.
//!
//! These macros are applied to a *target crate* while it runs under a Driftwatch
//! snapshot: annotating its operations, state, and inline checkpoints makes the
//! running code emit a [`WatchEvent`] stream into the runtime's thread-local
//! buffer (drained via [`take_events`]), and registers the annotated operations
//! and types into the link-time discovery registry. Annotated code never names
//! the `runtime` or `linkme` crates directly — the macros funnel every reference
//! through the hidden [`__rt`] re-export, so this facade is the single dependency
//! a target crate adds.
//!
//! This is the *extraction surface*, not a stabilized public API end users are
//! meant to build applications against yet; nothing prevents such use, but the
//! shape is driven by the extraction driver's needs and may change.
//!
//! # Public surface
//!
//! - Macros: [`watch_operation`], [`watch_dep`], [`watch_point`], and
//!   [`watch_input`].
//! - Runtime items users interact with directly: [`Value`], [`ToValue`],
//!   [`WatchEvent`], [`take_events`], and [`reset`].
//!
//! # Production gating
//!
//! Tracing is **off by default**: a plain build expands every macro to identity,
//! so the annotated items compile unchanged and emit nothing, with no registry
//! statics and no references to the runtime — production pays nothing. Building
//! (or depending) with the `trace` feature forwards to `annotations-macros/trace`
//! and turns on emission plus the link-time registry; this is exactly what the
//! extraction driver does while taking a snapshot.

pub use annotations_macros::{watch_dep, watch_input, watch_operation, watch_point};
pub use runtime::{ToValue, Value, WatchEvent, reset, take_events};

/// Hidden plumbing the generated macro code funnels through. Not part of the
/// stable public API — user code should never name it directly.
#[doc(hidden)]
pub mod __rt {
    pub use runtime::linkme;
    pub use runtime::*;
}
