//! `#[watch_operation]` input emission as the `conformance.operation.inputs`
//! kvlist attribute.
//!
//! Feature-matrix coverage: which parameters appear in the inputs kvlist.
//! - value params → one entry each, keyed by bare identifier;
//! - `#[watch_input("name")]` → renames the key;
//! - `&mut T` non-receiver params → INCLUDED, carrying the pre-call value
//!   captured at span open (later mutations are separate observations);
//! - `&T` shared-ref params → INCLUDED, carrying the referent's value;
//! - the `self` receiver → EXCLUDED.

mod common;

use annotations::{SpanName, Value, Watchable, reset, take_spans, watch_operation};
use common::{obs, op_attrs, result};

#[watch_operation(component = "annotations")]
fn scaled(n: i64, factor: i64) -> i64 {
    n * factor
}

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::needless_pass_by_value,
        reason = "identity (trace-off) form borrows `name`; the shape exercises a renamed String param"
    )
)]
#[watch_operation(component = "annotations")]
fn greet(#[watch_input("subject")] name: String) -> String {
    format!("hi {name}")
}

#[watch_operation(component = "annotations")]
fn accumulate(total: &mut i64, delta: i64) {
    *total += delta;
}

#[derive(Watchable)]
struct Counter {
    #[watchable]
    n: i64,
}

#[watch_operation(component = "annotations")]
fn bump(c: &mut Counter) {
    c.n += 1;
}

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "the `&i64` shape is deliberate: it exercises shared-ref param emission"
    )
)]
#[watch_operation(component = "annotations")]
fn observe(value: &i64) -> i64 {
    *value
}

struct Widget;

impl Widget {
    #[cfg_attr(
        not(feature = "trace"),
        allow(
            clippy::unused_self,
            reason = "the receiver is deliberately unused: it exercises receiver exclusion"
        )
    )]
    #[watch_operation(component = "annotations")]
    fn resize(&self, width: i64) -> i64 {
        width
    }
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn value_params_each_appear_in_the_inputs_kvlist() {
    reset();
    assert_eq!(scaled(3, 4), 12);
    let spans = take_spans();
    assert_eq!(spans[0].name, SpanName::Operation);
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "scaled",
            &[("n", Value::Integer(3)), ("factor", Value::Integer(4))]
        )
    );
    assert_eq!(spans[0].events, vec![result(12)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn watch_input_overrides_the_input_key() {
    reset();
    assert_eq!(greet("ada".to_string()), "hi ada");
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "greet",
            &[("subject", Value::String("ada".into()))]
        )
    );
    assert_eq!(spans[0].events, vec![result("hi ada")]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn mut_ref_param_captured_as_pre_call_input() {
    reset();
    let mut total = 10_i64;
    accumulate(&mut total, 5);
    assert_eq!(total, 15);
    // `total: &mut i64` is captured by its pre-call value (10) at span open;
    // `delta` is a value input. Keys are sorted (`delta` then `total`). The unit
    // return emits no completion event. The whole-value mutation of `total` is
    // out of scope (no deref-mutation observation).
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "accumulate",
            &[("delta", Value::Integer(5)), ("total", Value::Integer(10))]
        )
    );
    assert_eq!(spans[0].events, vec![]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn mut_struct_param_captures_pre_call_input_and_field_observation() {
    reset();
    let mut c = Counter { n: 3 };
    bump(&mut c);
    assert_eq!(c.n, 4);
    // The `&mut Counter` param is captured by its pre-call structural value
    // (`{"n": 3}`) at span open, and its field mutation is echoed as a
    // `c.n` observation of the new value (4).
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs(
            "annotations",
            "bump",
            &[(
                "c",
                Value::Map(std::collections::BTreeMap::from([(
                    "n".to_string(),
                    Value::Integer(3)
                )]))
            )]
        )
    );
    assert_eq!(spans[0].events, vec![obs("c.n", 4)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn shared_ref_param_is_included_by_value() {
    reset();
    let n = 7_i64;
    assert_eq!(observe(&n), 7);
    // A `&T` shared ref IS included, carrying the referent's value.
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "observe", &[("value", Value::Integer(7))])
    );
    assert_eq!(spans[0].events, vec![result(7)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn receiver_is_excluded_from_inputs() {
    reset();
    let w = Widget;
    assert_eq!(w.resize(64), 64);
    // No entry for `&self` — only the value param.
    let spans = take_spans();
    assert_eq!(
        spans[0].attributes,
        op_attrs("annotations", "resize", &[("width", Value::Integer(64))])
    );
    assert_eq!(spans[0].events, vec![result(64)]);
}
