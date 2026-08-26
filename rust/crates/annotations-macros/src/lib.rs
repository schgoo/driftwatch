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
//!   `#[watch_input]` helper attributes, which cannot stand alone on a param);
//! - `#[watch_input]` → the annotated statement unchanged;
//! - `watch_point!(…)` → `()`.
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
mod operation;
#[cfg(feature = "trace")]
mod point;
#[cfg(feature = "trace")]
mod shared;

use proc_macro::TokenStream;

/// `#[watch_operation]` — mark a function as an operation.
///
/// The operation name is the function name and the component is the annotated
/// crate's package name (`CARGO_PKG_NAME`). Expansion emits a `Run` marker, one event per value
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
        strip_watch_input(item)
    }
}

/// Strip the inert `#[watch_input(...)]` parameter attributes from an identity
/// (trace-off) expansion.
///
/// `#[watch_input]` is only ever meaningful as a helper consumed by an enclosing
/// `#[watch_operation]`; a bare attribute-macro cannot legally sit on a function
/// parameter. With `trace` off `#[watch_operation]` performs no instrumentation,
/// but it must still remove these helper attributes so the untouched item
/// compiles. Falls back to the input unchanged if it does not parse as a `fn`.
#[cfg(not(feature = "trace"))]
fn strip_watch_input(item: TokenStream) -> TokenStream {
    let Ok(mut func) = syn::parse::<syn::ItemFn>(item.clone()) else {
        return item;
    };
    for arg in &mut func.sig.inputs {
        if let syn::FnArg::Typed(pt) = arg {
            pt.attrs.retain(|a| !a.path().is_ident("watch_input"));
        }
    }
    quote::quote!(#func).into()
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
        quote::quote!(()).into()
    }
}
