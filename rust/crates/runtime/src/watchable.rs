//! The [`Watchable`] trait and the [`WatchableStruct`] marker.
//!
//! Types that expose annotated fields implement [`Watchable`] (typically via
//! `#[derive(Watchable)]`) to emit one event per field. The [`WatchableStruct`]
//! marker is implemented only by struct types so that return-value emission can
//! distinguish a struct return from an enum return (see
//! [`crate::ReturnEmit`]).

use crate::ToValue;

/// Emits one event per annotated field of `self`.
///
/// Implemented (typically via `#[derive(Watchable)]`) by structs and enums that
/// expose annotated fields. `prefix`, when set, namespaces each emitted field
/// name (for example `outer.inner`).
pub trait Watchable {
    /// Emit an event for each annotated field, optionally under `prefix`.
    fn emit_fields(&self, prefix: Option<&str>);
}

/// Marker implemented (via `#[derive(Watchable)]`) ONLY by struct types, never
/// enums.
///
/// It lets return-value emission distinguish a struct return (which emits both
/// its per-field events and a structured `$result`) from an enum return (which
/// emits only the tagged `$result`). See [`crate::ReturnEmit`].
pub trait WatchableStruct: Watchable + ToValue {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, emit_event_v, reset, take_events};

    struct Point {
        x: i64,
        y: i64,
    }

    impl Watchable for Point {
        fn emit_fields(&self, prefix: Option<&str>) {
            let key = |field: &str| match prefix {
                Some(p) => format!("{p}.{field}"),
                None => field.to_string(),
            };
            emit_event_v(&key("x"), Value::Integer(self.x));
            emit_event_v(&key("y"), Value::Integer(self.y));
        }
    }

    #[test]
    fn emit_fields_without_prefix() {
        reset();
        Point { x: 1, y: 2 }.emit_fields(None);
        let events = take_events();
        assert_eq!(events[0].name(), "x");
        assert_eq!(events[1].name(), "y");
    }

    #[test]
    fn emit_fields_with_prefix() {
        reset();
        Point { x: 1, y: 2 }.emit_fields(Some("p"));
        let events = take_events();
        assert_eq!(events[0].name(), "p.x");
        assert_eq!(events[1].name(), "p.y");
    }
}
