//! Procedural macros for the Driftwatch annotation surface.
//!
//! These macros mark the operations, dependencies, state, and inline
//! checkpoints of a target crate so that running it emits a behavioral trace.
//! With the `trace` feature enabled (the default via the `annotations` facade)
//! they expand into calls funneled through `::annotations::__rt` — user code
//! never references the `runtime` or `linkme` crates directly. Depend on the
//! `annotations` facade rather than this crate.
//!
//! # Production gating (`trace` feature)
//!
//! Every entry point is gated on the `trace` feature. **With `trace` on**, each
//! macro emits its full expansion (event emission, the `$crate`-funnel paths,
//! and link-time registry statics). **With `trace` off**, each macro expands to
//! identity, so the output carries ZERO `emit_*` calls, ZERO `linkme` registry
//! statics, and ZERO `__rt` references:
//!
//! - `#[watch_operation]` / `#[watch_input]` → the annotated item or statement
//!   unchanged;
//! - `watch_point!(…)` → `()`;
//! - `watch_component!(…)` → nothing.
//!
//! # Layout exception
//!
//! A `proc-macro` crate must declare its `#[proc_macro*]` entry points at the
//! crate root, so — unlike the workspace's other crates — this `lib.rs` carries
//! the thin entry functions in addition to module declarations. The real
//! expansion logic lives in the concern-scoped sibling modules; each entry point
//! is a thin, feature-gated delegate.

// The expansion logic is only reachable with the `trace` feature; gating the
// modules keeps a `--no-default-features` build free of dead code.
#[cfg(feature = "trace")]
mod body;
#[cfg(feature = "trace")]
mod component;
#[cfg(feature = "trace")]
mod operation;
#[cfg(feature = "trace")]
mod point;
#[cfg(feature = "trace")]
mod shared;

use proc_macro::TokenStream;

/// `#[watch_operation]` — mark a function as an operation.
///
/// The operation name is the function name and the component comes from
/// `watch_component!`. Expansion emits a `Run` marker, one event per value
/// parameter, a `$result` on every return path, echoes field mutations of
/// `self`/parameters, and registers the operation for discovery. Takes no
/// arguments.
#[proc_macro_attribute]
pub fn watch_operation(attr: TokenStream, item: TokenStream) -> TokenStream {
    #[cfg(feature = "trace")]
    {
        operation::expand(attr, item)
    }
    #[cfg(not(feature = "trace"))]
    {
        let _ = attr;
        item
    }
}

/// `#[watch_input("name")]` — override the event name a parameter emits under.
///
/// Consumed by the enclosing `#[watch_operation]`; identity in standalone
/// position.
#[proc_macro_attribute]
pub fn watch_input(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = attr;
    item
}

/// `watch_point!("name", &expr)` — emit one inline checkpoint event.
#[proc_macro]
pub fn watch_point(input: TokenStream) -> TokenStream {
    #[cfg(feature = "trace")]
    {
        point::expand(input)
    }
    #[cfg(not(feature = "trace"))]
    {
        let _ = input;
        "()".parse().expect("`()` is valid tokens")
    }
}

/// `watch_component!("name")` — declare the crate's default component.
#[proc_macro]
pub fn watch_component(input: TokenStream) -> TokenStream {
    #[cfg(feature = "trace")]
    {
        component::expand(input)
    }
    #[cfg(not(feature = "trace"))]
    {
        let _ = input;
        TokenStream::new()
    }
}
