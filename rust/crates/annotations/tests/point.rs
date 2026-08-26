//! `watch_point!("name", &expr)` — a single inline checkpoint event.
//!
//! Feature-matrix coverage: an inline checkpoint emits exactly one named
//! [`WatchEvent::Event`] (no `Run` marker).

mod common;

use annotations::{reset, take_events, watch_point};
use common::ev;

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn checkpoint_emits_one_event() {
    reset();
    let x = 42_i64;
    assert_eq!(x, 42);
    watch_point!("cp", &x);
    assert_eq!(take_events(), vec![ev("cp", 42)]);
}
