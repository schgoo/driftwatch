//! The `sequential` fixture: two annotated operations, one carrying a
//! `watch_point!` observation.
//!
//! Each `watch_operation` emits an operation span (component / name / inputs)
//! with deterministic sibling ordering, parent linking, the `observation` and
//! `result` events, and `Unset` → OK status promotion.

use annotations::{watch_operation, watch_point};

/// Computes an order subtotal (`unit_price * qty`) and records it as an inline
/// observation before returning it as the operation result.
#[must_use]
#[watch_operation(component = "golden.pricing")]
pub fn subtotal(unit_price: i64, qty: i64) -> i64 {
    let total = unit_price * qty;
    watch_point!("subtotal", &total);
    total
}

/// Adds a flat 10% tax to a base amount, returning the taxed total.
#[must_use]
#[watch_operation(component = "golden.pricing")]
pub fn with_tax(base: i64) -> i64 {
    base + base / 10
}
