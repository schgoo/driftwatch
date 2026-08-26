//! `#[watch_operation]` field-mutation echo.
//!
//! Feature-matrix coverage: assignments to a field of `self` or of a tracked
//! parameter emit an echo of the field's new value (from `field_mutation_emit`
//! in the macro's `body.rs`).
//! - `&mut self` with `self.field = …` → an echo named `field`;
//! - a compound `self.field += …` → likewise echoed;
//! - `param.field = …` on a `&mut SomeStruct` param → an echo named
//!   `param.field` (the `&mut` param is excluded from INPUT emission, but its
//!   field mutations are still tracked because it is a named parameter).

mod common;

use annotations::{reset, take_events, watch_operation};
use common::{ev, run};

struct Counter {
    count: i64,
    last: i64,
}

impl Counter {
    #[watch_operation]
    fn bump(&mut self, by: i64) {
        self.count += by;
        self.last = by;
    }
}

struct Config {
    level: i64,
}

#[watch_operation]
fn configure(cfg: &mut Config, level: i64) {
    cfg.level = level;
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn self_field_mutations_are_echoed_in_order() {
    reset();
    let mut c = Counter { count: 1, last: 0 };
    c.bump(4);
    assert_eq!(c.count, 5);
    assert_eq!(c.last, 4);
    // Echoes are appended after each mutating statement, in body order: the
    // compound `+=` on `count`, then the plain `=` on `last`.
    assert_eq!(
        take_events(),
        vec![
            run("bump"),
            ev("bump.by", 4_i64),
            ev("count", 5_i64),
            ev("last", 4_i64),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn mut_ref_param_field_mutation_is_echoed_as_param_dot_field() {
    reset();
    let mut cfg = Config { level: 0 };
    configure(&mut cfg, 9);
    assert_eq!(cfg.level, 9);
    // `cfg: &mut Config` emits no input event, but its field mutation is echoed
    // as `cfg.level`; the value param `level` is echoed as usual.
    assert_eq!(
        take_events(),
        vec![
            run("configure"),
            ev("configure.level", 9_i64),
            ev("cfg.level", 9_i64),
        ]
    );
}
