//! The error type returned by [`crate::resolve`].

use std::fmt;

/// A resolution failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum ResolveError {
    /// The resolver was built without the `ra` feature, so rust-analyzer-backed
    /// resolution is unavailable. Enable `resolver/ra` for real resolution.
    Unavailable,
    /// rust-analyzer failed to load the target workspace (or a source file
    /// within it could not be resolved). Carries a human-readable description.
    Load(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Unavailable => {
                f.write_str("static resolution is unavailable: build with the `ra` feature")
            }
            ResolveError::Load(msg) => {
                write!(f, "failed to load the target with rust-analyzer: {msg}")
            }
        }
    }
}

impl std::error::Error for ResolveError {}
