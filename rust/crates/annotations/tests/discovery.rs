//! The discovery envelope contains registered operations (with params/return)
//! alongside registered types.

use annotations::{watch_component, watch_operation};

watch_component!("discovery");

#[watch_operation]
fn compute(a: i64, b: String) -> i64 {
    let _ = &b;
    a
}

#[test]
fn discovery_json_lists_registered_operations() {
    let json = annotations::__rt::discovery_json();
    assert!(json.starts_with("{\"operations\":["));
    assert!(json.contains("\"name\":\"compute\""));
    assert!(json.contains("\"fn_name\":\"compute\""));
    assert!(json.contains("\"is_setup\":false"));
    assert!(json.contains("\"return_type\":\"i64\""));
    assert!(json.contains("[\"a\",\"i64\"]"));
    assert!(json.contains("[\"b\",\"String\"]"));
    assert!(json.contains("\"component\":\"discovery\""));
    assert!(json.contains("\"types\":["));
}
