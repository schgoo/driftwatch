//! `#[watch_dep("name")]` — per-argument input events plus response/error
//! emission around a real dependency call bound by a `let` inside an operation
//! body.
//!
//! The observed call is `i64::from_str_radix`, a standard-library function the
//! target crate neither owns nor annotates: `#[watch_dep]` needs nothing from the
//! callee beyond its arguments and a `Result` return, so it stands in for a real
//! dependency in another crate. Like `#[watch_operation]`'s parameters, each
//! argument is emitted individually — by identifier (`parse.text`), or
//! positionally for a non-identifier argument (`parse.arg1` for the `16`
//! literal).
//!
//! Feature-matrix coverage:
//! - `Ok` path → each input event then `name.response` bracket the real call;
//! - `Err` path → each input event then `name.error` (the dependency's own
//!   `Display`, captured verbatim);
//! - a trailing `?` on the call is supported: the `Result` is observed before
//!   the `?` unwraps, and on the `Err` path the `?` propagates the error out of
//!   the op (whose boundary still records it as a tagged `Err` `$result`);
//! - the real call always runs and its `Result` binds unchanged (no
//!   substitution, no `Default`).

mod common;

use annotations::{Value, reset, take_events, watch_operation};
use common::{ev, run};

#[watch_operation]
fn to_int(text: &str) -> i64 {
    #[watch_dep("parse")]
    let parsed = i64::from_str_radix(text, 16);
    parsed.unwrap_or(-1)
}

#[watch_operation]
fn to_int_try(text: &str) -> Result<i64, std::num::ParseIntError> {
    #[watch_dep("parse")]
    let n = i64::from_str_radix(text, 16)?;
    Ok(n)
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_ok_emits_each_input_then_response() {
    reset();
    assert_eq!(to_int("2a"), 42);
    assert_eq!(
        take_events(),
        vec![
            run("to_int"),
            ev("to_int.text", "2a"),
            ev("parse.text", "2a"),
            ev("parse.arg1", 16),
            ev("parse.response", 42),
            ev("$result", 42),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_err_emits_each_input_then_error() {
    reset();
    assert_eq!(to_int("zz"), -1);
    // Captured from the dependency itself so the assertion locks *what watch_dep
    // emitted* without coupling to libstd's exact wording.
    let dep_err = i64::from_str_radix("zz", 16).unwrap_err().to_string();
    assert_eq!(
        take_events(),
        vec![
            run("to_int"),
            ev("to_int.text", "zz"),
            ev("parse.text", "zz"),
            ev("parse.arg1", 16),
            ev("parse.error", dep_err),
            ev("$result", -1),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_ok_try() {
    reset();
    assert_eq!(to_int_try("2a"), Ok(42));
    // The `Result` op emits `$result` as a tagged `Ok` map.
    let mut ok = std::collections::BTreeMap::new();
    ok.insert("Ok".to_string(), Value::Integer(42));
    assert_eq!(
        take_events(),
        vec![
            run("to_int_try"),
            ev("to_int_try.text", "2a"),
            ev("parse.text", "2a"),
            ev("parse.arg1", 16),
            ev("parse.response", 42),
            ev("$result", Value::Map(ok)),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_err_try() {
    reset();
    // The `?` propagates the dependency's `Err`, so the op returns `Err`.
    to_int_try("zz").unwrap_err();
    // Captured from the dependency itself so the assertion locks *what watch_dep
    // emitted* without coupling to libstd's exact wording.
    let dep_err = i64::from_str_radix("zz", 16).unwrap_err().to_string();
    // `watch_dep` emits `parse.error` before the `?` unwraps; the `?` then
    // propagates the `Err` out of `to_int_try`, whose operation boundary still
    // records the return as a tagged `Err` `$result` (verified empirically).
    let mut err = std::collections::BTreeMap::new();
    err.insert("Err".to_string(), Value::String(dep_err.clone()));
    assert_eq!(
        take_events(),
        vec![
            run("to_int_try"),
            ev("to_int_try.text", "zz"),
            ev("parse.text", "zz"),
            ev("parse.arg1", 16),
            ev("parse.error", dep_err),
            ev("$result", Value::Map(err)),
        ]
    );
}
