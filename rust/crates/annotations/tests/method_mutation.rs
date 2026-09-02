//! `#[watch_operation]` field-mutation echo as `conformance.observation` events.
//!
//! Feature-matrix coverage: assignments to a field of `self` or of a tracked
//! parameter emit an observation of the field's new value (from
//! `field_mutation_emit` in the macro's `body.rs`).
//! - `&mut self` with `self.field = …` → an observation named `field`;
//! - a compound `self.field += …` → likewise observed;
//! - `param.field = …` on a `&mut SomeStruct` param → an observation named
//!   `param.field` (the `&mut` param is excluded from the inputs kvlist, but its
//!   field mutations are still tracked because it is a named parameter).

mod common;

use annotations::{Value, reset, take_spans, watch_operation};
use common::{obs, op_attrs};

struct Counter {
    count: i64,
    last: i64,
}

impl Counter {
    #[watch_operation(component = "annotations")]
    fn bump(&mut self, by: i64) {
        self.count += by;
        self.last = by;
    }
}

struct Config {
    level: i64,
}

#[watch_operation(component = "annotations")]
fn configure(cfg: &mut Config, level: i64) {
    cfg.level = level;
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn self_field_mutations_are_observed_in_order() {
    reset();
    let mut c = Counter { count: 1, last: 0 };
    c.bump(4);
    assert_eq!(c.count, 5);
    assert_eq!(c.last, 4);
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "bump", &[("by", Value::Integer(4))])
    );
    // Observations are appended after each mutating statement, in body order:
    // the compound `+=` on `count`, then the plain `=` on `last`. A unit return
    // adds no completion event.
    assert_eq!(spans[0].events, vec![obs("count", 5), obs("last", 4)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn mut_ref_param_field_mutation_is_observed_as_param_dot_field() {
    reset();
    let mut cfg = Config { level: 0 };
    configure(&mut cfg, 9);
    assert_eq!(cfg.level, 9);
    let spans = take_spans();
    // `cfg: &mut Config` is excluded from the inputs kvlist, but its field
    // mutation is observed as `cfg.level`; the value param `level` is an input.
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "configure", &[("level", Value::Integer(9))])
    );
    assert_eq!(spans[0].events, vec![obs("cfg.level", 9)]);
}
