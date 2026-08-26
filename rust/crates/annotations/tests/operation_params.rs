//! `#[watch_operation]` parameter (input) emission.
//!
//! Feature-matrix coverage: which parameters emit an `op.<name>` input event.
//! - value params → one event each;
//! - `#[watch_input("name")]` → renames the emitted event;
//! - `&mut T` params → EXCLUDED (threaded state, not an input);
//! - `&T` shared-ref params → INCLUDED, emitting the referent's value (locked by
//!   observation of `build_pre_stmts`/`is_mut_ref`/`is_printable_param`);
//! - the `self` receiver → EXCLUDED (a method with one value param emits no
//!   receiver event).

mod common;

use annotations::{reset, take_events, watch_operation};
use common::{ev, run};

#[watch_operation]
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
#[watch_operation]
fn greet(#[watch_input("subject")] name: String) -> String {
    format!("hi {name}")
}

#[watch_operation]
fn accumulate(total: &mut i64, delta: i64) {
    *total += delta;
}

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "the `&i64` shape is deliberate: it exercises shared-ref param emission"
    )
)]
#[watch_operation]
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
    #[watch_operation]
    fn resize(&self, width: i64) -> i64 {
        width
    }
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn value_params_each_emit_one_event() {
    reset();
    assert_eq!(scaled(3, 4), 12);
    assert_eq!(
        take_events(),
        vec![
            run("scaled"),
            ev("scaled.n", 3_i64),
            ev("scaled.factor", 4_i64),
            ev("$result", 12_i64),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn watch_input_overrides_param_event_name() {
    reset();
    assert_eq!(greet("ada".to_string()), "hi ada");
    assert_eq!(
        take_events(),
        vec![
            run("greet"),
            ev("greet.subject", "ada".to_string()),
            ev("$result", "hi ada".to_string()),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn mut_ref_param_is_excluded_from_input_emission() {
    reset();
    let mut total = 10_i64;
    accumulate(&mut total, 5);
    assert_eq!(total, 15);
    // `total: &mut i64` is state, not an input — only `delta` is echoed, and the
    // unit return emits no `$result`.
    assert_eq!(
        take_events(),
        vec![run("accumulate"), ev("accumulate.delta", 5_i64)]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn shared_ref_param_is_emitted_by_value() {
    reset();
    let n = 7_i64;
    assert_eq!(observe(&n), 7);
    // Observed behavior: a `&T` shared ref IS emitted, carrying the referent's
    // value (not skipped like `&mut T`).
    assert_eq!(
        take_events(),
        vec![
            run("observe"),
            ev("observe.value", 7_i64),
            ev("$result", 7_i64),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn receiver_emits_no_event() {
    reset();
    let w = Widget;
    assert_eq!(w.resize(64), 64);
    // No event for `&self` — only the value param and the result.
    assert_eq!(
        take_events(),
        vec![
            run("resize"),
            ev("resize.width", 64_i64),
            ev("$result", 64_i64),
        ]
    );
}
