//! The rust-analyzer resolution driver: load the target, then walk its source
//! selecting and resolving annotated operations and `Watchable` types.

use std::collections::BTreeSet;

use ra_ap_hir::{Semantics, attach_db};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::{AstNode, ast};
use ra_ap_vfs::FileId;

use crate::error::ResolveError;
use crate::load::{load_workspace, source_files};
use crate::named_type::{analyze_enum, analyze_struct};
use crate::resolve::Resolved;
use crate::select::{collect_watchable_names, is_watch_operation, is_watchable};
use crate::signature::analyze_function;
use crate::target::ResolveTarget;

/// Load `target` with rust-analyzer and resolve its annotated items.
pub(crate) fn resolve_with_ra(target: &ResolveTarget) -> Result<Resolved, ResolveError> {
    let (db, vfs) = load_workspace(target)?;
    let files = source_files(&vfs, target.root());
    // `attach_db` binds the db to the current thread so `HirDisplay` works.
    Ok(attach_db(&db, || resolve_files(&db, &files)))
}

/// Walk each target source file, resolving annotated operations and types.
fn resolve_files(db: &RootDatabase, files: &[(FileId, String)]) -> Resolved {
    let sema = Semantics::new(db);
    let known: BTreeSet<String> = collect_watchable_names(&sema, files);
    let mut operations = Vec::new();
    let mut types = Vec::new();

    for (file_id, _path) in files {
        let Some(editioned) = sema.attach_first_edition_opt(*file_id) else {
            continue;
        };
        let source = sema.parse(editioned);
        let Some(display_target) = sema
            .first_crate(*file_id)
            .map(|krate| krate.to_display_target(db))
        else {
            continue;
        };

        for f in source.syntax().descendants().filter_map(ast::Fn::cast) {
            if is_watch_operation(f.syntax()) {
                let hir_fn = sema.to_def(&f);
                operations.push(analyze_function(
                    db,
                    &sema,
                    display_target,
                    &known,
                    &f,
                    hir_fn,
                ));
            }
        }
        for s in source.syntax().descendants().filter_map(ast::Struct::cast) {
            if is_watchable(s.syntax())
                && let Some(ty) = analyze_struct(db, &sema, display_target, &known, &s)
            {
                types.push(ty);
            }
        }
        for e in source.syntax().descendants().filter_map(ast::Enum::cast) {
            if is_watchable(e.syntax())
                && let Some(ty) = analyze_enum(db, &sema, display_target, &known, &e)
            {
                types.push(ty);
            }
        }
    }

    Resolved { operations, types }
}
