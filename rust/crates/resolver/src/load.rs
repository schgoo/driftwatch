//! rust-analyzer workspace loading: turn a [`ResolveTarget`] into a loaded
//! [`RootDatabase`] + [`Vfs`], and enumerate the target's own source files.

use std::path::Path;

use ra_ap_ide_db::RootDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, RustLibSource};
use ra_ap_vfs::{FileId, Vfs};

use crate::error::ResolveError;
use crate::target::ResolveTarget;

/// Load the cargo workspace at the target with rust-analyzer.
///
/// Proc-macro expansion is disabled ([`ProcMacroServerChoice::None`]): with it
/// enabled, `Semantics::to_def` fails on `#[watch_operation]` items, so
/// selection scans source attributes and resolves pre-expansion syntax (see the
/// `PoC` / REPORT §1). The sysroot is discovered so std types resolve.
pub(crate) fn load_workspace(target: &ResolveTarget) -> Result<(RootDatabase, Vfs), ResolveError> {
    let cargo_config = CargoConfig {
        sysroot: Some(RustLibSource::Discover),
        ..CargoConfig::default()
    };
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::None,
        prefill_caches: false,
        num_worker_threads: 4,
        proc_macro_processes: 2,
    };
    let (db, vfs, _proc_macros) =
        load_workspace_at(target.root(), &cargo_config, &load_config, &|_msg| {})
            .map_err(|e| ResolveError::Load(e.to_string()))?;
    Ok((db, vfs))
}

/// The target's own `.rs` source files (VFS id + path), filtered to those under
/// the target root so std/dependency sources are excluded.
pub(crate) fn source_files(vfs: &Vfs, root: &Path) -> Vec<(FileId, String)> {
    let root_dir = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let prefix = root_dir.to_string_lossy().replace('\\', "/");
    vfs.iter()
        .filter_map(|(id, p)| p.as_path().map(|ap| (id, ap.to_string())))
        .filter(|(_, p)| {
            let normalized = p.replace('\\', "/");
            normalized.starts_with(&prefix)
                && Path::new(p)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
        })
        .collect()
}
