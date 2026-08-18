//! The thread-local mock table and its accessors.
//!
//! Annotated code can register deterministic lookup tables (mocks) so that a
//! dependency's output is replayed from a fixed map during extraction. The
//! table is thread-local, mirroring the trace buffer, and is cleared by
//! [`crate::reset`] via [`clear_mocks`].

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static MOCKS: RefCell<HashMap<String, HashMap<String, String>>> =
        RefCell::new(HashMap::new());
}

/// Register a mock lookup table under `mock_name`, replacing any existing entry.
///
/// `entries` is a list of `(input, output)` string pairs consulted by
/// [`mock_lookup`].
pub fn set_mock(mock_name: &str, entries: &[(&str, &str)]) {
    let mut map = HashMap::new();
    for (k, v) in entries {
        map.insert((*k).to_string(), (*v).to_string());
    }
    MOCKS.with(|m| {
        m.borrow_mut().insert(mock_name.to_string(), map);
    });
}

/// Look up `input` in the mock table registered under `mock_name`.
///
/// Returns `None` when the mock is unregistered or has no entry for `input`.
#[must_use]
pub fn mock_lookup(mock_name: &str, input: &str) -> Option<String> {
    MOCKS.with(|m| {
        m.borrow()
            .get(mock_name)
            .and_then(|t| t.get(input).cloned())
    })
}

/// Clear every registered mock. Called by [`crate::reset`] between operations.
pub(crate) fn clear_mocks() {
    MOCKS.with(|m| m.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_lookup() {
        clear_mocks();
        set_mock("db", &[("k1", "v1"), ("k2", "v2")]);
        assert_eq!(mock_lookup("db", "k1"), Some("v1".to_string()));
        assert_eq!(mock_lookup("db", "k2"), Some("v2".to_string()));
        assert_eq!(mock_lookup("db", "missing"), None);
        assert_eq!(mock_lookup("absent", "k1"), None);
        clear_mocks();
    }

    #[test]
    fn set_replaces_prior_table() {
        clear_mocks();
        set_mock("m", &[("a", "1")]);
        set_mock("m", &[("b", "2")]);
        assert_eq!(mock_lookup("m", "a"), None);
        assert_eq!(mock_lookup("m", "b"), Some("2".to_string()));
        clear_mocks();
    }

    #[test]
    fn clear_removes_all() {
        set_mock("m", &[("a", "1")]);
        clear_mocks();
        assert_eq!(mock_lookup("m", "a"), None);
    }
}
