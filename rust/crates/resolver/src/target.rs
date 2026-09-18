//! The resolution target: which crate the resolver aims rust-analyzer at.

use std::path::{Path, PathBuf};

/// A resolution target: the crate whose annotated items the resolver resolves.
///
/// rust-analyzer loads the cargo workspace rooted at [`ResolveTarget::root`],
/// then the resolver scans that crate's source for `#[watch_operation]` and
/// `#[derive(Watchable)]` items and resolves their signatures/definitions.
#[derive(Debug, Clone)]
pub struct ResolveTarget {
    /// The crate root (a directory holding a `Cargo.toml`, or the manifest
    /// path itself); rust-analyzer loads the workspace at this location.
    root: PathBuf,
}

impl ResolveTarget {
    /// Create a target rooted at `root` — a crate directory or its
    /// `Cargo.toml`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The crate root rust-analyzer loads the workspace at.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}
