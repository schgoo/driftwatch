//! The Driftwatch static resolver.
//!
//! `resolver` uses [rust-analyzer](https://rust-analyzer.github.io/) as a
//! library to statically resolve a target crate's annotated operations and
//! `#[derive(Watchable)]` types into the **already-resolved** contract IR:
//! [`contract::ResolvedOperation`] / [`contract::ResolvedType`]. It is the
//! type-aware replacement for the alias-blind string front-half in `extract`:
//! real type inference sees through `type` aliases and foreign `Result` aliases
//! (`io::Result`, `anyhow::Result`) that the string pipeline cannot, recovering
//! the hidden error channels.
//!
//! The output feeds straight into [`contract::assemble`]: each
//! [`Resolved::operations`] / [`Resolved::types`] entry carries its resolved
//! `component` tag, `is_setup` flag, and a fully alias-resolved
//! [`contract::Operation`] / [`contract::NamedType`]. Registry assembly (the
//! envelope, component grouping, sorting) stays in `contract`.
//!
//! ## Feature gate
//!
//! rust-analyzer and its heavy, pinned dependencies live behind the crate's
//! `ra` feature (default **off**), so they are absent from the default
//! workspace build. With `ra` off the crate still compiles — the API types are
//! present — and [`resolve`] returns [`ResolveError::Unavailable`]. With `ra`
//! on, resolution is real.
//!
//! ```
//! use resolver::{ResolveError, ResolveTarget, resolve};
//!
//! let target = ResolveTarget::new("path/to/crate");
//! let resolved = match resolve(&target) {
//!     Ok(resolved) => resolved,
//!     // Without the `ra` feature, resolution is unavailable — nothing to
//!     // assemble; a `Load` error is likewise a recoverable outcome to report.
//!     Err(ResolveError::Unavailable) => return,
//!     Err(err) => {
//!         eprintln!("resolution failed: {err}");
//!         return;
//!     }
//! };
//! let _doc = contract::assemble(
//!     &resolved.operations,
//!     &resolved.types,
//!     "urn:ctsc:registry:example:1".to_string(),
//!     "1.0.0".to_string(),
//! );
//! ```

mod error;
mod resolve;
mod target;

#[cfg(feature = "ra")]
mod component;
#[cfg(feature = "ra")]
mod driver;
#[cfg(feature = "ra")]
mod load;
#[cfg(feature = "ra")]
mod named_type;
#[cfg(feature = "ra")]
mod select;
#[cfg(feature = "ra")]
mod signature;
#[cfg(feature = "ra")]
mod type_map;

pub use error::ResolveError;
pub use resolve::{Resolved, resolve};
pub use target::ResolveTarget;
