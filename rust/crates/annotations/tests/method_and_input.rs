//! Field-mutation echo on a `&mut self` method, and the `#[watch_input]`
//! parameter-name override.

use annotations::{Value, WatchEvent, reset, take_events, watch_component, watch_operation};

watch_component!("counter");

struct Counter {
    count: i64,
}

impl Counter {
    #[watch_operation]
    fn bump(&mut self, by: i64) {
        self.count += by;
    }
}

#[watch_operation]
fn greet(#[watch_input("subject")] name: String) -> String {
    format!("hi {name}")
}

fn ev(name: &str, value: Value) -> WatchEvent {
    WatchEvent::Event {
        name: name.to_string(),
        value,
    }
}

fn run(name: &str) -> WatchEvent {
    WatchEvent::Run {
        operation: name.to_string(),
    }
}

#[test]
fn method_field_mutation_is_echoed() {
    reset();
    let mut c = Counter { count: 1 };
    c.bump(4);
    assert_eq!(c.count, 5);
    assert_eq!(
        take_events(),
        vec![
            run("bump"),
            ev("bump.by", Value::Integer(4)),
            ev("count", Value::Integer(5)),
        ]
    );
}

#[test]
fn watch_input_overrides_param_event_name() {
    reset();
    assert_eq!(greet("ada".to_string()), "hi ada");
    assert_eq!(
        take_events(),
        vec![
            run("greet"),
            ev("greet.subject", Value::String("ada".into())),
            ev("$result", Value::String("hi ada".into())),
        ]
    );
}
