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

/// A bare local checkpointed under the semantic name `"level"`.
#[watch_operation(component = "annotations")]
fn bare_level(level: i64) -> i64 {
    watch_point!("level", &level);
    level
}

/// The same semantic name `"level"`, but the value now lives behind a struct
/// field (`cfg.level`) — a representation change the name is meant to survive.
#[watch_operation(component = "annotations")]
fn struct_level(level: i64) -> i64 {
    struct Config {
        level: i64,
    }
    let cfg = Config { level };
    watch_point!("level", &cfg.level);
    cfg.level
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
    assert_eq!(x, 7);
}

/// Semantic naming is representation-independent: `watch_point!("level", …)`
/// produces the same observation identity (name AND value) whether the value is
/// a bare local (`&level`) or a struct field (`&cfg.level`). The name binds to
/// the author's intent, not to how the value happens to be spelled — so a
/// refactor that moves `level` under a struct is not spurious drift (see
/// `docs/value-canonicalization.md`, the semantic-naming escape hatch).
#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn semantic_name_is_representation_independent() {
    reset();
    assert_eq!(bare_level(3), 3);
    let bare_spans = take_spans();
    let bare_obs = bare_spans[0].events[0].clone();

    reset();
    assert_eq!(struct_level(3), 3);
    let struct_spans = take_spans();
    let struct_obs = struct_spans[0].events[0].clone();

    // Both carry the same semantic name and the same value...
    assert_eq!(bare_obs, obs("level", 3));
    assert_eq!(struct_obs, obs("level", 3));
    // ...so the identity is stable across the `level` → `cfg.level` change.
    assert_eq!(bare_obs, struct_obs);
}
