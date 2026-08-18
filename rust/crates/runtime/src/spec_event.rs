//! The [`SpecEvent`] trait and the [`SpecEventStruct`] marker.
//!
//! Types that expose annotated fields implement [`SpecEvent`] (typically via
//! `#[derive(SpecEvent)]`) to emit one event per field. The [`SpecEventStruct`]
//! marker is implemented only by struct types so that return-value emission can
//! distinguish a struct return from an enum return (see
//! [`crate::ReturnEmit`]).

use crate::ToValue;

/// Emits one trace event per annotated field of `self`.
///
/// Implemented (typically via `#[derive(SpecEvent)]`) by structs and enums that
/// expose annotated fields. `prefix`, when set, namespaces each emitted field
/// name (for example `outer.inner`).
pub trait SpecEvent {
    /// Emit an event for each annotated field, optionally under `prefix`.
    fn emit_fields(&self, prefix: Option<&str>);
}

/// Marker implemented (via `#[derive(SpecEvent)]`) ONLY by struct types, never
/// enums.
///
/// It lets return-value emission distinguish a struct return (which emits both
/// its per-field events and a structured `$result`) from an enum return (which
/// emits only the tagged `$result`). See [`crate::ReturnEmit`].
pub trait SpecEventStruct: SpecEvent + ToValue {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, emit_event_v, reset, take_traces};

    struct Point {
        x: i64,
        y: i64,
    }

    impl SpecEvent for Point {
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
        let traces = take_traces();
        assert_eq!(traces[0].name(), "x");
        assert_eq!(traces[1].name(), "y");
    }

    #[test]
    fn emit_fields_with_prefix() {
        reset();
        Point { x: 1, y: 2 }.emit_fields(Some("p"));
        let traces = take_traces();
        assert_eq!(traces[0].name(), "p.x");
        assert_eq!(traces[1].name(), "p.y");
    }
}
