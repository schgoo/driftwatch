//! Body instrumentation for `#[watch_operation]`: field-mutation echo and
//! dispatch to the `#[watch_dep]` `let`-rewrite.
//!
//! The [`BodyInstrumenter`] walks the operation body, recursing into nested
//! blocks first. It rewrites each `#[watch_dep]`-tagged `let` (see [`crate::dep`])
//! and, after any statement that mutates a field of `self` or of a value
//! parameter (`self.x = …`, `param.x += …`), inserts an event echoing the new
//! field value.

use syn::visit_mut::VisitMut;
use syn::{BinOp, Block, Expr, Stmt, parse_quote};

use crate::dep::rewrite_local;
use crate::shared::rt;

/// Rewrites an operation body in place: expands `#[watch_dep]` bindings and
/// appends field-mutation echo events.
pub struct BodyInstrumenter {
    /// The value-parameter names whose field mutations are echoed.
    pub param_names: Vec<String>,
    /// The enclosing operation's declared component, threaded into each
    /// `#[watch_dep]` so a nested-operation span that omits its own `component`
    /// inherits the parent's.
    pub component: String,
}

impl VisitMut for BodyInstrumenter {
    #[allow(
        clippy::renamed_function_params,
        reason = "descriptive name for the visited block"
    )]
    fn visit_block_mut(&mut self, block: &mut Block) {
        // Recurse into nested blocks first.
        for stmt in &mut block.stmts {
            syn::visit_mut::visit_stmt_mut(self, stmt);
        }

        let original = std::mem::take(&mut block.stmts);
        let mut new: Vec<Stmt> = Vec::with_capacity(original.len());

        for stmt in original {
            match stmt {
                Stmt::Local(local) => new.extend(rewrite_local(local, &self.component)),
                stmt => {
                    let emit_after = field_mutation_emit(&stmt, &self.param_names);
                    new.push(stmt);
                    if let Some(after) = emit_after {
                        new.push(after);
                    }
                }
            }
        }

        block.stmts = new;
    }
}

/// If `stmt` assigns (or compound-assigns) to a field of `self` or of a tracked
/// value parameter, build the event that echoes the new field value.
fn field_mutation_emit(stmt: &Stmt, param_names: &[String]) -> Option<Stmt> {
    let Stmt::Expr(expr, Some(_)) = stmt else {
        return None;
    };

    let lhs = match expr {
        Expr::Assign(a) => &*a.left,
        Expr::Binary(b) => {
            let is_compound = matches!(
                b.op,
                BinOp::AddAssign(_)
                    | BinOp::SubAssign(_)
                    | BinOp::MulAssign(_)
                    | BinOp::DivAssign(_)
                    | BinOp::RemAssign(_)
                    | BinOp::BitXorAssign(_)
                    | BinOp::BitAndAssign(_)
                    | BinOp::BitOrAssign(_)
                    | BinOp::ShlAssign(_)
                    | BinOp::ShrAssign(_)
            );
            if !is_compound {
                return None;
            }
            &*b.left
        }
        _ => return None,
    };
    field_emit_from_lhs(lhs, param_names)
}

fn field_emit_from_lhs(lhs: &Expr, param_names: &[String]) -> Option<Stmt> {
    let Expr::Field(field) = lhs else {
        return None;
    };
    let syn::Member::Named(id) = &field.member else {
        return None;
    };
    let field_name = id.to_string();
    let event_name = match &*field.base {
        Expr::Path(p) if p.path.is_ident("self") => field_name.clone(),
        Expr::Path(p) => {
            let id = p.path.get_ident()?;
            let name = id.to_string();
            if !param_names.contains(&name) {
                return None;
            }
            format!("{name}.{field_name}")
        }
        _ => return None,
    };
    let rt = rt();
    let stmt: Stmt = parse_quote! {
        #rt::push_observation(#event_name, #rt::ToValue::to_value(&(#lhs)));
    };
    Some(stmt)
}
