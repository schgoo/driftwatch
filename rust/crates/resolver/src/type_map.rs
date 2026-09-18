//! The alias→[`contract::TypeRef`] mapping: RA type inference lowered into the
//! contract IR, including error-name recovery (trace-contract producer-choice
//! #4 — a foreign or owned error collapses to its last path segment).

use std::collections::BTreeSet;

use contract::{ErrorOutcome, Outcomes, Primitive, TypeRef};
use ra_ap_hir::{DisplayTarget, HirDisplay, Type};
use ra_ap_ide_db::RootDatabase;

/// Classify a function's fully-inferred return type into completion outcomes.
///
/// `Result<T, E>` yields `result: T` (or, for `Result<Option<T>, E>`,
/// `result: T` + `empty`) plus a single name-only error outcome recovered from
/// `E`. `Option<T>` yields `result: T` + `empty`. Anything else is a bare
/// result. Because RA has already resolved aliases, `io::Result<T>` and
/// `anyhow::Result<T>` arrive here as plain `Result<T, E>` — their error
/// channels recovered where the string pipeline saw them as infallible.
pub(crate) fn classify_return_ra(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ty: &Type<'_>,
) -> Outcomes {
    if ty.is_unit() {
        return Outcomes::default();
    }
    if let Some((adt, args)) = ty.as_adt_with_args() {
        let head = adt.name(db).display(db, display_target.edition).to_string();
        if head == "Result" {
            let ok = args.first().and_then(|x| x.as_ref());
            let err = args.get(1).and_then(|x| x.as_ref());
            let mut result = ok.map(|t| type_ref_from_ra(db, display_target, known, t));
            let mut empty = false;
            if let Some(ok_ty) = ok
                && let Some((ok_adt, ok_args)) = ok_ty.as_adt_with_args()
                && ok_adt
                    .name(db)
                    .display(db, display_target.edition)
                    .to_string()
                    == "Option"
            {
                result = ok_args
                    .first()
                    .and_then(|x| x.as_ref())
                    .map(|t| type_ref_from_ra(db, display_target, known, t));
                empty = true;
            }
            let errors = err
                .map(|e| {
                    vec![ErrorOutcome {
                        name: error_name(db, display_target, e),
                        ty: None,
                        description: None,
                    }]
                })
                .unwrap_or_default();
            return Outcomes {
                result,
                empty,
                errors,
            };
        }
        if head == "Option" {
            return Outcomes {
                result: args
                    .first()
                    .and_then(|x| x.as_ref())
                    .map(|t| type_ref_from_ra(db, display_target, known, t)),
                empty: true,
                errors: Vec::new(),
            };
        }
    }
    Outcomes {
        result: Some(type_ref_from_ra(db, display_target, known, ty)),
        empty: false,
        errors: Vec::new(),
    }
}

/// Lower a fully-inferred RA type into a contract [`TypeRef`].
///
/// Primitives and the standard collection heads map structurally; a named type
/// declared in the target (`known`) or any other ADT maps to
/// [`TypeRef::Named`]. Aliases are already resolved by RA, so this sees through
/// them.
pub(crate) fn type_ref_from_ra(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ty: &Type<'_>,
) -> TypeRef {
    let display = type_display(db, display_target, ty);
    if let Some(primitive) = primitive_from_display(&display) {
        return TypeRef::Primitive { name: primitive };
    }
    if let Some((adt, args)) = ty.as_adt_with_args() {
        let head = adt.name(db).display(db, display_target.edition).to_string();
        match head.as_str() {
            "Vec" | "VecDeque" => {
                return TypeRef::List {
                    items: Box::new(first_arg_ref(db, display_target, known, &args)),
                };
            }
            "Option" => {
                return TypeRef::Optional {
                    value: Box::new(first_arg_ref(db, display_target, known, &args)),
                };
            }
            "HashSet" | "BTreeSet" => {
                return TypeRef::Set {
                    items: Box::new(first_arg_ref(db, display_target, known, &args)),
                };
            }
            "HashMap" | "BTreeMap" => {
                return TypeRef::Map {
                    keys: Box::new(nth_arg_ref(db, display_target, known, &args, 0)),
                    values: Box::new(nth_arg_ref(db, display_target, known, &args, 1)),
                };
            }
            _ => {
                return TypeRef::Named {
                    name: head,
                    component_id: None,
                    registry_id: None,
                };
            }
        }
    }
    let tuple_args: Vec<TypeRef> = ty
        .type_arguments()
        .map(|t| type_ref_from_ra(db, display_target, known, &t))
        .collect();
    if display.starts_with('(') && !tuple_args.is_empty() {
        return TypeRef::Tuple { items: tuple_args };
    }
    TypeRef::Named {
        name: display,
        component_id: None,
        registry_id: None,
    }
}

/// The recovered error name for an error type: its last path segment (an ADT's
/// simple name — `io::Error`/`anyhow::Error` → `"Error"`), per producer-choice
/// #4. Non-ADT error types fall back to their display form.
pub(crate) fn error_name(
    db: &RootDatabase,
    display_target: DisplayTarget,
    ty: &Type<'_>,
) -> String {
    if let Some((adt, _)) = ty.as_adt_with_args() {
        adt.name(db).display(db, display_target.edition).to_string()
    } else {
        type_display(db, display_target, ty)
    }
}

/// The RA display form of a type (used for primitive/tuple detection).
fn type_display(db: &RootDatabase, display_target: DisplayTarget, ty: &Type<'_>) -> String {
    ty.display(db, display_target).to_string()
}

/// Map an RA type-display string to a contract primitive, if it names one.
fn primitive_from_display(display: &str) -> Option<Primitive> {
    Some(match display {
        "()" => Primitive::Unit,
        "String" | "std::string::String" | "alloc::string::String" | "str" | "&str" => {
            Primitive::String
        }
        "bool" => Primitive::Bool,
        "i32" => Primitive::I32,
        "i64" | "isize" | "i8" | "i16" | "i128" => Primitive::I64,
        "u32" => Primitive::U32,
        "u64" | "usize" | "u8" | "u16" | "u128" => Primitive::U64,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        _ => return None,
    })
}

/// The `contract::TypeRef` of the first generic argument, or `unit` if absent.
fn first_arg_ref(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    args: &[Option<Type<'_>>],
) -> TypeRef {
    nth_arg_ref(db, display_target, known, args, 0)
}

/// The `contract::TypeRef` of the `n`-th generic argument, or `unit` if absent.
fn nth_arg_ref(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    args: &[Option<Type<'_>>],
    n: usize,
) -> TypeRef {
    args.get(n).and_then(|x| x.as_ref()).map_or(
        TypeRef::Primitive {
            name: Primitive::Unit,
        },
        |t| type_ref_from_ra(db, display_target, known, t),
    )
}
