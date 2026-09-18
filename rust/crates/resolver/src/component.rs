//! Component-id resolution: the annotation's `component = "…"` selector, or the
//! item's module path / owning crate, mapped to a registry component tag.

use ra_ap_hir::{DisplayTarget, Module};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::SyntaxNode;

/// The component tag for an annotated item.
///
/// Precedence (per the `PoC` / annotation convention):
/// 1. the attribute's explicit `component = "…"` selector, if present;
/// 2. otherwise the first segment of the item's HIR module path;
/// 3. otherwise (a crate-root item) the owning crate's name.
pub(crate) fn component_for(
    db: &RootDatabase,
    display_target: DisplayTarget,
    attr_node: &SyntaxNode,
    module: Option<Module>,
) -> String {
    if let Some(explicit) = attr_component(attr_node) {
        return explicit;
    }
    if let Some(module) = module {
        let path = module_path(db, module, display_target);
        if let Some(first) = path.split("::").next()
            && !first.is_empty()
        {
            return first.to_owned();
        }
        if let Some(name) = module.krate(db).display_name(db) {
            return name.to_string();
        }
    }
    "crate".to_owned()
}

/// The `::`-joined HIR module path of `module` (empty for the crate root).
pub(crate) fn module_path(
    db: &RootDatabase,
    module: Module,
    display_target: DisplayTarget,
) -> String {
    module
        .path_segments(db)
        .map(|s| s.display(db, display_target.edition).to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// The explicit `component = "…"` selector on the item's attributes, if any.
pub(crate) fn attr_component(node: &SyntaxNode) -> Option<String> {
    component_arg(&node.text().to_string())
}

/// Extract the string literal following a `component` marker in attribute text.
fn component_arg(text: &str) -> Option<String> {
    let marker = "component";
    let i = text.find(marker)?;
    first_string_literal(&text[i + marker.len()..])
}

/// The first double-quoted string literal in `text`, unquoted.
fn first_string_literal(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}
