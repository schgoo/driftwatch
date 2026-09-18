//! Source-attribute selection: scan the target's syntax (pre-expansion) for the
//! `#[watch_operation]` / `#[derive(Watchable)]` items to resolve.
//!
//! Selection is deliberately source-level, not HIR-level: with proc-macro
//! expansion disabled, attribute-macro items cannot be joined back through
//! `Semantics::to_def` by their expansion, so we match on the source attribute
//! text and resolve the underlying signature/definition (REPORT §1).

use std::collections::BTreeSet;

use ra_ap_hir::Semantics;
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::ast::HasName;
use ra_ap_syntax::{AstNode, SyntaxNode, ast};
use ra_ap_vfs::FileId;

/// The set of `#[derive(Watchable)]` struct/enum names declared across the
/// target's source, so operation signatures can flag references to them as
/// named types.
pub(crate) fn collect_watchable_names(
    sema: &Semantics<'_, RootDatabase>,
    files: &[(FileId, String)],
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (file_id, _) in files {
        let Some(editioned) = sema.attach_first_edition_opt(*file_id) else {
            continue;
        };
        let source = sema.parse(editioned);
        for s in source.syntax().descendants().filter_map(ast::Struct::cast) {
            if is_watchable(s.syntax())
                && let Some(n) = s.name()
            {
                out.insert(n.to_string());
            }
        }
        for e in source.syntax().descendants().filter_map(ast::Enum::cast) {
            if is_watchable(e.syntax())
                && let Some(n) = e.name()
            {
                out.insert(n.to_string());
            }
        }
    }
    out
}

/// Whether an item's attributes select it as a watched operation.
pub(crate) fn is_watch_operation(node: &SyntaxNode) -> bool {
    has_attr_text(node, "watch_operation")
}

/// Whether an item's attributes select it as a `Watchable` type.
pub(crate) fn is_watchable(node: &SyntaxNode) -> bool {
    has_attr_text(node, "derive(Watchable") || has_attr_text(node, "Watchable")
}

/// Whether `node`'s source text contains `needle` (a cheap attribute probe).
fn has_attr_text(node: &SyntaxNode, needle: &str) -> bool {
    node.text().to_string().contains(needle)
}
