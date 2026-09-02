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
//! - `#[watch_operation]` → the annotated item unchanged (bar removing the inert
//!   `#[watch_input]` / `#[watch_dep]` helper attributes, which cannot stand
//!   alone on a param or a statement);
//! - `#[watch_input]` / `#[watch_dep]` → the annotated statement unchanged;
//! - `watch_point!(…)` → `()`;
//! - `#[derive(Watchable)]` → nothing (no impls, no registry statics).
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
mod dep;
#[cfg(feature = "trace")]
mod operation;
#[cfg(feature = "trace")]
mod point;
#[cfg(feature = "trace")]
mod shared;
#[cfg(feature = "trace")]
mod watchable;

use proc_macro::TokenStream;

/// `#[watch_operation(component = "...")]` — mark a function as an operation.
///
/// The operation name is the function name and the component is author-declared
/// (mandatory: a missing `component` is a `compile_error!`). Expansion opens a
/// `conformance.operation` span (inputs as one kvlist attribute), pushes a
/// completion event (`result`/`empty`/`error`) on every return path, echoes
/// field mutations of `self`/parameters as `conformance.observation` events, and
/// registers the operation for discovery.
#[proc_macro_attribute]
pub fn watch_operation(attr: TokenStream, item: TokenStream) -> TokenStream {
    #[cfg(feature = "trace")]
    {
        operation::expand(attr, item)
    }
    #[cfg(not(feature = "trace"))]
    {
        let _ = attr;
        strip_helper_attrs(item)
    }
}

/// Strip the inert `#[watch_input]` / `#[watch_dep]` helper attributes from an
/// identity (trace-off) expansion.
///
/// `#[watch_input]` (on a value parameter) and `#[watch_dep]` (on a `let` in the
/// body) are only ever meaningful as helpers consumed by an enclosing
/// `#[watch_operation]`; a bare attribute-macro cannot legally sit on a
/// parameter or a statement. With `trace` off `#[watch_operation]` performs no
/// instrumentation, but it must still remove these helper attributes so the
/// untouched item compiles. Falls back to the input unchanged if it does not
/// parse as a `fn`.
#[cfg(not(feature = "trace"))]
fn strip_helper_attrs(item: TokenStream) -> TokenStream {
    use syn::visit_mut::VisitMut;

    let Ok(mut func) = syn::parse::<syn::ItemFn>(item.clone()) else {
        return item;
    };
    for arg in &mut func.sig.inputs {
        if let syn::FnArg::Typed(pt) = arg {
            pt.attrs.retain(|a| !a.path().is_ident("watch_input"));
        }
    }
    StripWatchDep.visit_block_mut(&mut func.block);
    quote::quote!(#func).into()
}

/// Removes `#[watch_dep]` from every `let` binding in the body (including nested
/// blocks and closures) for the trace-off identity expansion.
#[cfg(not(feature = "trace"))]
struct StripWatchDep;

#[cfg(not(feature = "trace"))]
impl syn::visit_mut::VisitMut for StripWatchDep {
    #[allow(
        clippy::renamed_function_params,
        reason = "descriptive name for the visited local"
    )]
    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        local.attrs.retain(|a| !a.path().is_ident("watch_dep"));
        syn::visit_mut::visit_local_mut(self, local);
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

/// `#[watch_dep("name", component = "...")]` — observe a dependency call bound
/// by a `let` inside a `#[watch_operation]` body.
///
/// Consumed and rewritten by the enclosing `#[watch_operation]`, which opens a
/// nested `conformance.operation` span (own inputs + completion) around the real
/// call; `component` is optional and defaults to the enclosing operation's.
/// Identity in standalone position, in both configurations.
#[proc_macro_attribute]
pub fn watch_dep(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = attr;
    item
}

/// `#[derive(Watchable)]` — decompose a type to a `Value` and register it.
///
/// Structs decompose each `#[watchable]`-tagged field (honoring
/// `#[watchable(name = "…")]`) to a `Value::Map`; enums decompose to a
/// `Value::Variant` per variant. Off-trace the derive expands to nothing (no
/// impls, no registry).
#[proc_macro_derive(Watchable, attributes(watchable))]
pub fn derive_watchable(input: TokenStream) -> TokenStream {
    #[cfg(feature = "trace")]
    {
        watchable::expand(input)
    }
    #[cfg(not(feature = "trace"))]
    {
        let _ = input;
        TokenStream::new()
    }
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
        quote::quote!(()).into()
    }
}
