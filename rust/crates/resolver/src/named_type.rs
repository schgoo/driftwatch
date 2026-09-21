//! Struct/enum → [`contract::ResolvedType`]: decompose a `#[derive(Watchable)]`
//! type into a contract record (struct) or tagged union (enum).

use std::collections::BTreeSet;

use contract::{Field, NamedType, ResolvedType, TypeRef, Variant};
use ra_ap_hir::{DisplayTarget, Semantics};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::ast;

use crate::component::crate_component;
use crate::type_map::type_ref_from_ra;

/// Resolve one `#[derive(Watchable)]` struct into a record [`ResolvedType`].
pub(crate) fn analyze_struct(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_struct: &ast::Struct,
) -> Option<ResolvedType> {
    let def = sema.to_def(ast_struct)?;
    let name = def.name(db).display(db, display_target.edition).to_string();
    let fields = def
        .fields(db)
        .into_iter()
        .map(|field| Field {
            name: field
                .name(db)
                .display(db, display_target.edition)
                .to_string(),
            ty: type_ref_from_ra(db, display_target, known, &field.ty(db)),
            description: None,
        })
        .collect();
    Some(ResolvedType {
        component: crate_component(db, def.module(db)),
        named_type: NamedType::Record {
            name,
            fields,
            description: None,
        },
    })
}

/// Resolve one `#[derive(Watchable)]` enum into a tagged-union [`ResolvedType`].
///
/// Each variant with named/tuple fields carries an inline record payload; a
/// unit variant carries none.
pub(crate) fn analyze_enum(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_enum: &ast::Enum,
) -> Option<ResolvedType> {
    let def = sema.to_def(ast_enum)?;
    let name = def.name(db).display(db, display_target.edition).to_string();
    let variants = def
        .variants(db)
        .into_iter()
        .map(|v| {
            let fields: Vec<Field> = v
                .fields(db)
                .into_iter()
                .map(|field| Field {
                    name: field
                        .name(db)
                        .display(db, display_target.edition)
                        .to_string(),
                    ty: type_ref_from_ra(db, display_target, known, &field.ty(db)),
                    description: None,
                })
                .collect();
            Variant {
                name: v.name(db).display(db, display_target.edition).to_string(),
                payload: (!fields.is_empty()).then_some(TypeRef::Record { fields }),
                description: None,
            }
        })
        .collect();
    Some(ResolvedType {
        component: crate_component(db, def.module(db)),
        named_type: NamedType::TaggedUnion {
            name,
            variants,
            description: None,
        },
    })
}
