//! Function → [`contract::ResolvedOperation`]: resolve an annotated operation's
//! signature (inputs + completion outcomes) through rust-analyzer.

use std::collections::BTreeSet;

use contract::{NamedValue, Operation, Outcomes, Primitive, ResolvedOperation, TypeRef};
use ra_ap_hir::{DisplayTarget, Function, Semantics};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::ast::HasName;
use ra_ap_syntax::{AstNode, ast};

use crate::component::component_for;
use crate::type_map::{classify_return_ra, type_ref_from_ra};

/// Resolve one `#[watch_operation]` function into a [`ResolvedOperation`].
///
/// Inputs come from the parameter list (skipping `self`); the return type is
/// classified into completion outcomes with alias-resolved result/error types.
/// Observations and cross-component dependencies are deferred — the acceptance
/// oracle carries none, and robust `watch_point!`/`watch_dep!` token-tree →
/// expression mapping is not achievable with proc-macros disabled (spike REPORT
/// §3–§4 flags both PARTIAL), so they stay empty here.
pub(crate) fn analyze_function(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_fn: &ast::Fn,
    hir_fn: Option<Function>,
) -> ResolvedOperation {
    let name = hir_fn
        .map(|h| h.name(db).display(db, display_target.edition).to_string())
        .or_else(|| ast_fn.name().map(|n| n.to_string()))
        .unwrap_or_else(|| "unknown".to_owned());
    let component = component_for(
        db,
        display_target,
        ast_fn.syntax(),
        hir_fn.map(|h| h.module(db)),
    );
    let inputs = inputs(db, sema, display_target, known, ast_fn, hir_fn);
    let outcomes = outcomes(db, sema, display_target, known, ast_fn, hir_fn);

    ResolvedOperation {
        component,
        // TODO(R-d): the annotation macros do not yet mark setups; wire the
        // `is_setup` flag when the setup annotation lands.
        is_setup: false,
        operation: Operation {
            name,
            description: None,
            inputs,
            // TODO(R-e or later): `watch_point!` observations are PARTIAL with
            // proc-macros disabled (spike REPORT §3); leave empty for now.
            observations: Vec::new(),
            outcomes,
        },
    }
}

/// Resolve the operation's inputs, preferring the source-parameter type (so
/// alias spellings resolve) and falling back to the HIR parameter type.
fn inputs(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_fn: &ast::Fn,
    hir_fn: Option<Function>,
) -> Vec<NamedValue> {
    let ast_params: Vec<ast::Param> = ast_fn
        .param_list()
        .into_iter()
        .flat_map(|pl| pl.params())
        .collect();
    let hir_params = hir_fn
        .map(|h| h.params_without_self(db))
        .unwrap_or_default();
    ast_params
        .into_iter()
        .enumerate()
        .map(|(idx, ast_param)| {
            let name = ast_param
                .pat()
                .map_or_else(|| format!("arg{idx}"), |p| p.syntax().text().to_string());
            let ty = ast_param
                .ty()
                .and_then(|ast_ty| sema.resolve_type(&ast_ty))
                .map(|resolved| type_ref_from_ra(db, display_target, known, &resolved))
                .or_else(|| {
                    hir_params
                        .get(idx)
                        .map(|p| type_ref_from_ra(db, display_target, known, p.ty()))
                })
                .unwrap_or(TypeRef::Primitive {
                    name: Primitive::Unit,
                });
            NamedValue {
                name,
                ty,
                description: None,
            }
        })
        .collect()
}

/// Resolve the operation's completion outcomes from its return type.
fn outcomes(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_fn: &ast::Fn,
    hir_fn: Option<Function>,
) -> Outcomes {
    let ret_from_ast = ast_fn
        .ret_type()
        .and_then(|rt| rt.ty())
        .and_then(|ast_ty| sema.resolve_type(&ast_ty));
    let ret_from_hir = hir_fn.map(|h| h.ret_type(db));
    ret_from_ast
        .as_ref()
        .or(ret_from_hir.as_ref())
        .map(|r| classify_return_ra(db, display_target, known, r))
        .unwrap_or_default()
}
