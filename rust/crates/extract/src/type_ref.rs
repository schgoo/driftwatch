//! Parse a stringified Rust type (as captured in `runtime::OpMeta`/`TypeMeta`)
//! into a language-neutral [`contract::TypeRef`] (registry.md §5).
//!
//! The runtime registry stores raw, stringified signature types — e.g.
//! `"HashMap<String, Vec<u8>>"` or `"Option<User>"`. This module lowers that
//! string domain into the resolved CTSC type-reference model with a small
//! balanced-bracket tokenizer, so nested generics and whitespace variants map
//! correctly. Anything it cannot classify degrades to a best-effort
//! [`TypeRef::Named`] or [`TypeRef::Primitive`] rather than panicking: a
//! malformed type string must never abort the build-time derivation.

use std::collections::BTreeSet;

use contract::{Primitive, TypeRef};

/// Parse a stringified Rust type into a [`contract::TypeRef`].
///
/// `known` is the set of declared type names (final path segments) in scope;
/// it is threaded through recursion so nested references resolve against the
/// same declaration set. Unrecognized paths become [`TypeRef::Named`] (whose
/// resolution against `known` is enforced later by [`contract::validate`]).
///
/// Recognized shapes: `Vec<T>`/`VecDeque<T>` → list, `Option<T>` → optional,
/// `HashMap`/`BTreeMap<K, V>` → map, `HashSet`/`BTreeSet<T>` → set,
/// `Box`/`Rc`/`Arc<T>` → transparent inner, tuples → tuple, scalar idents →
/// primitive, everything else → named (final segment only).
#[must_use]
pub(crate) fn parse_type_ref(s: &str, known: &BTreeSet<&str>) -> TypeRef {
    let s = strip_refs(s.trim());

    // Tuple (and the unit `()`), before any generic handling.
    if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = split_top_level(inner, ',')
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect();
        return match parts.as_slice() {
            [] => unit(),
            // `(T)` is just `T`; a genuine 1-tuple `(T,)` collapses here too,
            // an accepted fidelity loss for the v1 driver.
            [single] => parse_type_ref(single, known),
            _ => TypeRef::Tuple {
                items: parts.iter().map(|p| parse_type_ref(p, known)).collect(),
            },
        };
    }

    if let Some((base, args)) = split_generic(s) {
        let base = last_segment(base);
        return match (base, args.as_slice()) {
            ("Vec" | "VecDeque", [item]) => TypeRef::List {
                items: Box::new(parse_type_ref(item, known)),
            },
            ("HashSet" | "BTreeSet", [item]) => TypeRef::Set {
                items: Box::new(parse_type_ref(item, known)),
            },
            ("Option", [value]) => TypeRef::Optional {
                value: Box::new(parse_type_ref(value, known)),
            },
            ("HashMap" | "BTreeMap", [keys, values]) => TypeRef::Map {
                keys: Box::new(parse_type_ref(keys, known)),
                values: Box::new(parse_type_ref(values, known)),
            },
            // Transparent smart-pointer wrappers: peel to the inner value.
            ("Box" | "Rc" | "Arc", [inner]) => parse_type_ref(inner, known),
            _ => named(base),
        };
    }

    let seg = last_segment(s);
    match primitive(seg) {
        Some(name) => TypeRef::Primitive { name },
        // A declared type resolves to its registry name (final segment); an
        // unresolved reference is passed through verbatim so it dangles
        // visibly under `contract::validate` (§8) rather than being silently
        // normalized into a different name.
        None if known.contains(seg) => named(seg),
        None => named(s),
    }
}

/// The `unit` primitive `TypeRef`.
fn unit() -> TypeRef {
    TypeRef::Primitive {
        name: Primitive::Unit,
    }
}

/// A best-effort named reference to `seg`, scoped to the current component.
fn named(seg: &str) -> TypeRef {
    TypeRef::Named {
        name: seg.to_string(),
        component_id: None,
        registry_id: None,
    }
}

/// Map a scalar identifier to its CTSC [`Primitive`], if any.
///
/// The CTSC primitive set is narrower than Rust's, so widths collapse:
/// `i8`/`i16`/`i32` → `I32`, `i64`/`i128`/`isize` → `I64`, and likewise for the
/// unsigned family. `char` has no CTSC scalar and maps to `String`.
fn primitive(seg: &str) -> Option<Primitive> {
    Some(match seg {
        "String" | "str" | "char" => Primitive::String,
        "bool" => Primitive::Bool,
        "i8" | "i16" | "i32" => Primitive::I32,
        "i64" | "i128" | "isize" => Primitive::I64,
        "u8" | "u16" | "u32" => Primitive::U32,
        "u64" | "u128" | "usize" => Primitive::U64,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        _ => return None,
    })
}

/// Strip leading reference markers (`&`, `&mut`) and lifetimes from a type
/// string, returning the referent. `&'a str` and `& str` both yield `str`.
fn strip_refs(mut s: &str) -> &str {
    while let Some(rest) = s.strip_prefix('&') {
        s = rest.trim_start();
        if let Some(rest) = s.strip_prefix('\'') {
            s = rest
                .trim_start_matches(|c: char| c.is_alphanumeric() || c == '_')
                .trim_start();
        }
        if let Some(rest) = s.strip_prefix("mut ") {
            s = rest.trim_start();
        }
    }
    s
}

/// The final `::`-separated path segment, trimmed.
fn last_segment(s: &str) -> &str {
    s.rsplit("::").next().unwrap_or(s).trim()
}

/// Split a generic type `Base<A, B, ...>` into its base path and top-level
/// argument list. Returns `None` when `s` is not an angle-bracketed generic.
fn split_generic(s: &str) -> Option<(&str, Vec<&str>)> {
    let open = s.find('<')?;
    let inner = s.strip_suffix('>')?.get(open + 1..)?;
    let base = s[..open].trim();
    if base.is_empty() {
        return None;
    }
    Some((base, split_top_level(inner, ',')))
}

/// Split `s` on top-level occurrences of `delim`, ignoring separators nested
/// inside `<>`, `()`, or `[]`. Each returned slice is trimmed.
fn split_top_level(s: &str, delim: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            c if c == delim && depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(s[start..].trim());
    parts
}

/// Base path and argument list of a generic type string, with the base reduced
/// to its final segment. Used by the return-type classifier.
pub(crate) fn generic_parts(s: &str) -> Option<(&str, Vec<&str>)> {
    let (base, args) = split_generic(s.trim())?;
    Some((last_segment(base), args))
}

/// The final path segment of a (possibly generic) type string, generics
/// stripped. Used to name error outcomes from a `Result<_, E>` error type.
pub(crate) fn type_head(s: &str) -> &str {
    let s = s.trim();
    last_segment(s.split('<').next().unwrap_or(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> BTreeSet<&'static str> {
        ["User", "Receipt"].into_iter().collect()
    }

    fn parse(s: &str) -> TypeRef {
        parse_type_ref(s, &known())
    }

    fn prim(p: Primitive) -> TypeRef {
        TypeRef::Primitive { name: p }
    }

    #[test]
    fn primitives_map_to_scalars() {
        assert_eq!(parse("i32"), prim(Primitive::I32));
        assert_eq!(parse("i64"), prim(Primitive::I64));
        assert_eq!(parse("u8"), prim(Primitive::U32));
        assert_eq!(parse("u64"), prim(Primitive::U64));
        assert_eq!(parse("f64"), prim(Primitive::F64));
        assert_eq!(parse("bool"), prim(Primitive::Bool));
        assert_eq!(parse("String"), prim(Primitive::String));
        assert_eq!(parse("char"), prim(Primitive::String));
        assert_eq!(parse("()"), prim(Primitive::Unit));
    }

    #[test]
    fn reference_and_lifetime_strip_to_referent() {
        assert_eq!(parse("&str"), prim(Primitive::String));
        assert_eq!(parse("&'a str"), prim(Primitive::String));
        assert_eq!(parse("& str"), prim(Primitive::String));
        assert_eq!(parse("&mut String"), prim(Primitive::String));
    }

    #[test]
    fn custom_paths_become_named_final_segment() {
        assert_eq!(
            parse("crate::model::User"),
            TypeRef::Named {
                name: "User".to_string(),
                component_id: None,
                registry_id: None,
            }
        );
    }

    #[test]
    fn vec_of_option_nests() {
        assert_eq!(
            parse("Vec<Option<User>>"),
            TypeRef::List {
                items: Box::new(TypeRef::Optional {
                    value: Box::new(TypeRef::Named {
                        name: "User".to_string(),
                        component_id: None,
                        registry_id: None,
                    }),
                }),
            }
        );
    }

    #[test]
    fn hashmap_string_vec_u8_splits_on_top_level_comma() {
        assert_eq!(
            parse("HashMap<String, Vec<u8>>"),
            TypeRef::Map {
                keys: Box::new(prim(Primitive::String)),
                values: Box::new(TypeRef::List {
                    items: Box::new(prim(Primitive::U32)),
                }),
            }
        );
    }

    #[test]
    fn btree_set_maps_to_set() {
        assert_eq!(
            parse("BTreeSet<i64>"),
            TypeRef::Set {
                items: Box::new(prim(Primitive::I64)),
            }
        );
    }

    #[test]
    fn tuple_positions_are_typed() {
        assert_eq!(
            parse("(i32, String, User)"),
            TypeRef::Tuple {
                items: vec![
                    prim(Primitive::I32),
                    prim(Primitive::String),
                    TypeRef::Named {
                        name: "User".to_string(),
                        component_id: None,
                        registry_id: None,
                    },
                ],
            }
        );
    }

    #[test]
    fn single_paren_group_is_transparent() {
        assert_eq!(parse("(User)"), parse("User"));
    }

    #[test]
    fn whitespace_variants_are_tolerated() {
        assert_eq!(
            parse("  HashMap < String , Vec < u8 > >  "),
            parse("HashMap<String, Vec<u8>>")
        );
    }

    #[test]
    fn smart_pointers_are_transparent() {
        assert_eq!(parse("Box<User>"), parse("User"));
        assert_eq!(parse("Arc<i32>"), prim(Primitive::I32));
    }

    #[test]
    fn nested_map_of_list_of_tuple() {
        assert_eq!(
            parse("BTreeMap<String, Vec<(u8, u8)>>"),
            TypeRef::Map {
                keys: Box::new(prim(Primitive::String)),
                values: Box::new(TypeRef::List {
                    items: Box::new(TypeRef::Tuple {
                        items: vec![prim(Primitive::U32), prim(Primitive::U32)],
                    }),
                }),
            }
        );
    }

    #[test]
    fn generic_custom_type_reduces_to_named_base() {
        assert_eq!(
            parse("Wrapper<User>"),
            TypeRef::Named {
                name: "Wrapper".to_string(),
                component_id: None,
                registry_id: None,
            }
        );
    }

    #[test]
    fn unresolved_full_path_is_passed_through_verbatim() {
        // `std::time::Duration` is not declared, so it is kept verbatim (and
        // will dangle under validate) rather than normalized to `Duration`.
        assert_eq!(
            parse("std::time::Duration"),
            TypeRef::Named {
                name: "std::time::Duration".to_string(),
                component_id: None,
                registry_id: None,
            }
        );
    }

    #[test]
    fn type_head_strips_generics_and_path() {
        assert_eq!(type_head("std::num::ParseIntError"), "ParseIntError");
        assert_eq!(type_head("MyErr<T>"), "MyErr");
    }
}
