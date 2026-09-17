//! Classify a stringified return type into CTSC completion [`Outcomes`]
//! (registry.md §4.1).
//!
//! This is the string-domain analogue of the annotation macros'
//! `classify_return`/`result_ok_type`/`result_err_type` (see
//! `annotations-macros/src/shared.rs`), operating on the raw `return_type`
//! string captured in `runtime::OpMeta` rather than on a parsed `syn::Type`.
//! Keeping it string-based lets the extraction driver derive outcomes without
//! depending on `annotations-macros`.
//!
//! Mapping:
//! - `Result<T, E>` → `{ result: Some(T), empty: false, errors: [E] }`
//! - `Result<Option<T>, E>` → `{ result: Some(T), empty: true, errors: [E] }`
//! - `Option<T>` → `{ result: Some(T), empty: true, errors: [] }`
//! - `()` (or empty) → `{ result: None, empty: false, errors: [] }`
//! - bare `T` → `{ result: Some(T), empty: false, errors: [] }`
//!
//! An error outcome carries only its name — the last path segment of `E` — in
//! v1; the decomposed error payload type is left absent so it never dangles
//! against the declared type set.

use std::collections::BTreeSet;

use contract::{ErrorOutcome, Outcomes};

use crate::type_ref::{generic_parts, parse_type_ref, type_head};

/// Classify `return_type` into CTSC completion [`Outcomes`].
///
/// `known` is forwarded to [`parse_type_ref`] for the success/result type.
#[must_use]
pub(crate) fn classify_return(return_type: &str, known: &BTreeSet<&str>) -> Outcomes {
    let s = return_type.trim();
    if s.is_empty() || s == "()" {
        return Outcomes::default();
    }

    match generic_parts(s) {
        Some(("Result", args)) => {
            let ok = args.first().copied().unwrap_or("()");
            let errors = args
                .get(1)
                .map(|err| {
                    vec![ErrorOutcome {
                        name: type_head(err).to_string(),
                        ty: None,
                        description: None,
                    }]
                })
                .unwrap_or_default();
            // Peel a nested `Option<T>` in the Ok position → `empty`.
            match generic_parts(ok) {
                Some(("Option", inner)) => Outcomes {
                    result: Some(parse_type_ref(
                        inner.first().copied().unwrap_or("()"),
                        known,
                    )),
                    empty: true,
                    errors,
                },
                _ => Outcomes {
                    result: Some(parse_type_ref(ok, known)),
                    empty: false,
                    errors,
                },
            }
        }
        Some(("Option", args)) => Outcomes {
            result: Some(parse_type_ref(args.first().copied().unwrap_or("()"), known)),
            empty: true,
            errors: Vec::new(),
        },
        _ => Outcomes {
            result: Some(parse_type_ref(s, known)),
            empty: false,
            errors: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::{Primitive, TypeRef};

    fn known() -> BTreeSet<&'static str> {
        ["User", "Receipt", "ChargeError"].into_iter().collect()
    }

    fn classify(s: &str) -> Outcomes {
        classify_return(s, &known())
    }

    fn prim(p: Primitive) -> TypeRef {
        TypeRef::Primitive { name: p }
    }

    #[test]
    fn unit_has_no_result() {
        assert_eq!(classify("()"), Outcomes::default());
        assert_eq!(classify(""), Outcomes::default());
    }

    #[test]
    fn bare_type_is_plain_result() {
        assert_eq!(
            classify("i64"),
            Outcomes {
                result: Some(prim(Primitive::I64)),
                empty: false,
                errors: Vec::new(),
            }
        );
    }

    #[test]
    fn option_is_result_with_empty() {
        assert_eq!(
            classify("Option<User>"),
            Outcomes {
                result: Some(TypeRef::Named {
                    name: "User".to_string(),
                    component_id: None,
                    registry_id: None,
                }),
                empty: true,
                errors: Vec::new(),
            }
        );
    }

    #[test]
    fn result_has_result_and_error_name() {
        let out = classify("Result<Receipt, ChargeError>");
        assert!(!out.empty);
        assert_eq!(
            out.result,
            Some(TypeRef::Named {
                name: "Receipt".to_string(),
                component_id: None,
                registry_id: None,
            })
        );
        assert_eq!(out.errors.len(), 1);
        assert_eq!(out.errors[0].name, "ChargeError");
        assert!(out.errors[0].ty.is_none());
    }

    #[test]
    fn result_option_peels_to_empty() {
        let out = classify("Result<Option<User>, ChargeError>");
        assert!(out.empty);
        assert_eq!(
            out.result,
            Some(TypeRef::Named {
                name: "User".to_string(),
                component_id: None,
                registry_id: None,
            })
        );
        assert_eq!(out.errors[0].name, "ChargeError");
    }

    #[test]
    fn error_name_uses_last_path_segment() {
        let out = classify("Result<i64, std::num::ParseIntError>");
        assert_eq!(out.errors[0].name, "ParseIntError");
    }
}
