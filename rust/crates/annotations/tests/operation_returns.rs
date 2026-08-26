//! `#[watch_operation]` return (`$result`) shapes.
//!
//! Feature-matrix coverage: `$result` emission across return kinds.
//! - unit / `()` → NO `$result` event;
//! - printable scalar → a direct `$result`;
//! - `Result<T, E>` → a tagged `{Ok|Err}` map `$result`;
//! - `Option<T>` → a tagged `{Some|None}` map `$result`;
//! - `async fn` → `$result` emitted after the awaited body completes.
//!
//! (Struct/enum returns via the `ReturnEmit` ladder need `#[derive(Watchable)]`
//! — covered in #29.)

mod common;

use annotations::{Value, reset, take_events, watch_operation};
use common::{ev, run};

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::needless_pass_by_value,
        reason = "identity (trace-off) form borrows `msg`; the shape exercises a unit-return op with a value param"
    )
)]
#[watch_operation]
fn log_only(msg: String) {
    let _ = msg;
}

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

#[cfg_attr(
    not(feature = "trace"),
    allow(
        clippy::unused_async,
        reason = "identity (trace-off) form has no await; the traced form wraps the body in an awaited async block"
    )
)]
#[watch_operation]
async fn doubled(id: i64) -> i64 {
    id * 2
}

/// A minimal executor: the operations under test are immediately ready, so a
/// noop-waker poll loop suffices without pulling in an async runtime.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    loop {
        if let std::task::Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn unit_return_emits_no_result() {
    reset();
    log_only("hello".to_string());
    assert_eq!(
        take_events(),
        vec![run("log_only"), ev("log_only.msg", "hello".to_string())]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn scalar_return_is_a_direct_result() {
    reset();
    assert_eq!(add(2, 3), 5);
    assert_eq!(
        take_events(),
        vec![
            run("add"),
            ev("add.a", 2_i64),
            ev("add.b", 3_i64),
            ev("$result", 5_i64),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn result_ok_is_tagged_ok_map() {
    reset();
    assert_eq!(checked_div(10, 2), Ok(5));
    let mut m = std::collections::BTreeMap::new();
    m.insert("Ok".to_string(), Value::Integer(5));
    assert_eq!(
        take_events(),
        vec![
            run("checked_div"),
            ev("checked_div.a", 10_i64),
            ev("checked_div.b", 2_i64),
            ev("$result", Value::Map(m)),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn result_err_is_tagged_err_map() {
    reset();
    assert_eq!(checked_div(1, 0), Err("divide by zero".to_string()));
    let mut m = std::collections::BTreeMap::new();
    m.insert("Err".to_string(), Value::String("divide by zero".into()));
    assert_eq!(
        take_events(),
        vec![
            run("checked_div"),
            ev("checked_div.a", 1_i64),
            ev("checked_div.b", 0_i64),
            ev("$result", Value::Map(m)),
        ]
    );
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
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
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
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

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn async_result_is_emitted_after_the_body_completes() {
    reset();
    assert_eq!(block_on(doubled(21)), 42);
    assert_eq!(
        take_events(),
        vec![
            run("doubled"),
            ev("doubled.id", 21_i64),
            ev("$result", 42_i64),
        ]
    );
}
