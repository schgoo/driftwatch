//! The public [`resolve`] entry point and its [`Resolved`] output.

use crate::error::ResolveError;
use crate::target::ResolveTarget;

/// The resolved contract IR for a target: alias-resolved operations and types,
/// ready to feed into [`contract::assemble`].
#[derive(Debug)]
pub struct Resolved {
    /// The resolved operations, each tagged with its component and `is_setup`.
    pub operations: Vec<contract::ResolvedOperation>,
    /// The resolved named types (records / tagged unions), each component-tagged.
    pub types: Vec<contract::ResolvedType>,
}

/// Resolve a target's annotated operations and types into the contract IR.
///
/// With the `ra` feature enabled, this loads the target with rust-analyzer and
/// resolves each annotated item's signature/definition. Without it, resolution
/// is unavailable and this returns [`ResolveError::Unavailable`].
///
/// # Errors
///
/// Returns [`ResolveError::Unavailable`] when built without the `ra` feature,
/// or [`ResolveError::Load`] when rust-analyzer cannot load the target.
#[cfg(feature = "ra")]
pub fn resolve(target: &ResolveTarget) -> Result<Resolved, ResolveError> {
    crate::driver::resolve_with_ra(target)
}

/// Resolve a target's annotated operations and types into the contract IR.
///
/// This build has no `ra` feature, so static resolution is unavailable.
///
/// # Errors
///
/// Always returns [`ResolveError::Unavailable`]; enable `resolver/ra` for real
/// resolution.
#[cfg(not(feature = "ra"))]
pub fn resolve(_target: &ResolveTarget) -> Result<Resolved, ResolveError> {
    Err(ResolveError::Unavailable)
}
