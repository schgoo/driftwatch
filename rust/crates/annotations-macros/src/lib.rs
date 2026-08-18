//! Procedural macros for the Driftwatch annotation surface.
//!
//! These macros mark the operations, state, and inline checkpoints of a target
//! crate so that running it emits a behavioral trace. They expand into calls
//! into the `driftwatch-runtime` emitter; user code never references the runtime
//! directly. Depend on the `driftwatch-annotations` facade rather than this
//! crate — it re-exports these macros alongside the runtime items they need.
