//! `watch_point!("name", &expr)` — a single inline checkpoint event.

use annotations::{Value, WatchEvent, reset, take_events, watch_point};

#[test]
fn checkpoint_emits_one_event() {
    reset();
    let x = 42_i64;
    watch_point!("cp", &x);
    assert_eq!(
        take_events(),
        vec![WatchEvent::Event {
            name: "cp".to_string(),
            value: Value::Integer(42),
        }]
    );
}
