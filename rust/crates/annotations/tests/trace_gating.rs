//! Production gating: the same annotated code emits under `trace` and is inert
//! (identity expansion, no events) with the default (no-`trace`) build.
//!
//! Run the off path with `cargo test -p annotations` (default features) and the
//! on path with `cargo test -p annotations --all-features`. Unlike the other
//! emission tests this one is NOT `#[ignore]`d off-trace: it asserts the correct
//! behavior in BOTH configs via `cfg`.

use annotations::{reset, take_events, watch_operation, watch_point};

#[watch_operation]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[test]
fn trace_feature_toggles_emission() {
    reset();
    let sum = add(2, 3);
    assert_eq!(sum, 5);
    watch_point!("cp", &sum);
    let events = take_events();

    #[cfg(feature = "trace")]
    assert!(!events.is_empty(), "trace on: annotated code must emit");

    #[cfg(not(feature = "trace"))]
    assert!(events.is_empty(), "trace off: annotated code must be inert");
}
