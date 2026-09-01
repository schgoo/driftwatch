//! Production gating: the same annotated code (including a `#[watch_dep]`
//! binding) emits under `trace` and expands to inert identity (no events)
//! with the default no-`trace` build.
//!
//! Run the off path with `cargo test -p annotations`, the on path with
//! `--all-features`. Asserts both configs via `cfg` (not `#[ignore]`d).

use annotations::{reset, take_events, watch_operation, watch_point};

#[watch_operation]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[watch_operation]
fn narrow(raw: i64) -> i64 {
    #[watch_dep("convert")]
    let byte = u8::try_from(raw);
    byte.map_or(-1, i64::from)
}

#[test]
fn trace_feature_toggles_emission() {
    reset();
    let sum = add(2, 3);
    assert_eq!(sum, 5);
    watch_point!("cp", &sum);
    assert_eq!(narrow(30), 30);
    let events = take_events();

    #[cfg(feature = "trace")]
    assert!(!events.is_empty(), "trace on: annotated code must emit");

    #[cfg(not(feature = "trace"))]
    assert!(events.is_empty(), "trace off: annotated code must be inert");
}
