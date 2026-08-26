//! `#[watch_operation]` on free functions: `Run` + param + `$result` emission,
//! and the tagged `Result`/`Option` `$result` shapes.

use annotations::{Value, WatchEvent, reset, take_events, watch_component, watch_operation};

watch_component!("calc");

#[watch_operation]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[watch_operation]
fn checked_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        return Err("divide by zero".to_string());
    }
    Ok(a / b)
}

#[watch_operation]
fn first_even(xs: Vec<i64>) -> Option<i64> {
    xs.into_iter().find(|n| n % 2 == 0)
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
fn scalar_operation_emits_run_params_and_result() {
    reset();
    assert_eq!(add(2, 3), 5);
    assert_eq!(
        take_events(),
        vec![
            run("add"),
            ev("add.a", Value::Integer(2)),
            ev("add.b", Value::Integer(3)),
            ev("$result", Value::Integer(5)),
        ]
    );
}

#[test]
fn result_ok_is_tagged_ok_map() {
    reset();
    assert_eq!(checked_div(10, 2), Ok(5));
    let mut m = std::collections::BTreeMap::new();
    m.insert("Ok".to_string(), Value::Integer(5));
    assert_eq!(
        take_events(),
        vec![
            run("checked_div"),
            ev("checked_div.a", Value::Integer(10)),
            ev("checked_div.b", Value::Integer(2)),
            ev("$result", Value::Map(m)),
        ]
    );
}

#[test]
fn result_err_is_tagged_err_map() {
    reset();
    assert_eq!(checked_div(1, 0), Err("divide by zero".to_string()));
    let mut m = std::collections::BTreeMap::new();
    m.insert("Err".to_string(), Value::String("divide by zero".into()));
    assert_eq!(
        take_events(),
        vec![
            run("checked_div"),
            ev("checked_div.a", Value::Integer(1)),
            ev("checked_div.b", Value::Integer(0)),
            ev("$result", Value::Map(m)),
        ]
    );
}

#[test]
fn option_some_is_tagged_some_map() {
    reset();
    assert_eq!(first_even(vec![1, 4, 6]), Some(4));
    let mut m = std::collections::BTreeMap::new();
    m.insert("Some".to_string(), Value::Integer(4));
    assert_eq!(
        take_events(),
        vec![
            run("first_even"),
            ev(
                "first_even.xs",
                Value::List(vec![
                    Value::Integer(1),
                    Value::Integer(4),
                    Value::Integer(6)
                ])
            ),
            ev("$result", Value::Map(m)),
        ]
    );
}

#[test]
fn option_none_is_tagged_none_map() {
    reset();
    assert_eq!(first_even(vec![1, 3]), None);
    let mut m = std::collections::BTreeMap::new();
    m.insert(
        "None".to_string(),
        Value::Map(std::collections::BTreeMap::new()),
    );
    assert_eq!(
        take_events(),
        vec![
            run("first_even"),
            ev(
                "first_even.xs",
                Value::List(vec![Value::Integer(1), Value::Integer(3)])
            ),
            ev("$result", Value::Map(m)),
        ]
    );
}
