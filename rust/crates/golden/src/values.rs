//! The `values` fixture: one annotated operation whose `watch_point!`
//! observations pack every CTSC §8 value type and edge, sourced from real typed
//! values through the runtime `ToValue` path.
//!
//! Value constructors are written as fully-qualified paths (rather than `use`
//! imports) so the trace-off build — where `watch_point!` expands to `()` and
//! discards its argument — has nothing unused to warn about.

use annotations::{watch_operation, watch_point};

/// Emits one observation per CTSC §8 value class and edge case.
///
/// Covers string/bool scalars; the integer edges (`0`, `±`, `i64::MIN`,
/// `i64::MAX`); the finite floats (fractional, whole, `-0.0`) and the
/// non-finite floats (`±Inf` and two distinct NaN bit-patterns that both
/// canonicalize to `"NaN"`); empty and nested lists; empty and out-of-order
/// sets (proving sorted emission); empty and nested maps; and a tagged-union
/// variant with and without a payload.
#[must_use]
#[watch_operation(component = "golden.values")]
pub fn value_edges() -> i64 {
    // Scalars.
    watch_point!("string", &"hello".to_string());
    watch_point!("bool", &true);

    // Integer edges.
    watch_point!("int_zero", &0_i64);
    watch_point!("int_negative", &(-7_i64));
    watch_point!("int_positive", &7_i64);
    watch_point!("int_min", &i64::MIN);
    watch_point!("int_max", &i64::MAX);

    // Finite floats.
    watch_point!("float_fractional", &1.5_f64);
    watch_point!("float_whole", &2.0_f64);
    watch_point!("float_negative_zero", &(-0.0_f64));

    // Non-finite floats; both NaN bit-patterns canonicalize to "NaN".
    watch_point!("float_infinity", &f64::INFINITY);
    watch_point!("float_neg_infinity", &f64::NEG_INFINITY);
    watch_point!("float_nan", &f64::NAN);
    watch_point!("float_nan_alt", &f64::from_bits(0x7ff8_0000_0000_0001));

    // Lists: empty + nested.
    watch_point!("list_empty", &Vec::<i64>::new());
    watch_point!("list_nested", &vec![vec![1_i64, 2], vec![3_i64]]);

    // Sets: empty + out-of-order (emitted in sorted order).
    watch_point!("set_empty", &std::collections::BTreeSet::<i64>::new());
    watch_point!(
        "set_unordered",
        &std::collections::BTreeSet::from([3_i64, 1, 2])
    );

    // Maps: empty + nested.
    watch_point!(
        "map_empty",
        &std::collections::BTreeMap::<String, i64>::new()
    );
    watch_point!(
        "map_nested",
        &std::collections::BTreeMap::from([(
            "outer".to_string(),
            std::collections::BTreeMap::from([("inner".to_string(), 1_i64)]),
        )])
    );

    // Variant: payload + unit (via the `impl ToValue for Value` identity).
    watch_point!(
        "variant_payload",
        &annotations::Value::variant("Wrapped", annotations::Value::Integer(3))
    );
    watch_point!("variant_unit", &annotations::Value::variant_unit("Active"));

    0
}
