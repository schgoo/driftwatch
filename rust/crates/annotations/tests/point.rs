//! `watch_point!("name", &expr)` — an inline `conformance.observation` event.
//!
//! Feature-matrix coverage: an inline checkpoint emits one observation event on
//! the current operation span; a bare `watch_point!` outside any operation emits
//! nothing (there is no span for it to land on).

mod common;

use annotations::{reset, take_spans, watch_operation, watch_point};
use common::{obs, result};

#[watch_operation(component = "annotations")]
fn checkpoints(x: i64) -> i64 {
    watch_point!("cp", &x);
    x
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn checkpoint_emits_one_observation_on_the_operation_span() {
    reset();
    assert_eq!(checkpoints(42), 42);
    let spans = take_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].events, vec![obs("cp", 42), result(42)]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn checkpoint_outside_an_operation_emits_nothing() {
    reset();
    let x = 7_i64;
    watch_point!("cp", &x);
    assert!(take_spans().is_empty());
}
