//! The golden harness tests: one focused scenario per test case, each named
//! `<subject>_<condition>_<result>`. Every test drives one reusable fixture,
//! frames it with a run/scenario span, serializes the drained capture, and
//! byte-compares against its on-disk golden under `tests/golden/`.
//!
//! Compare: `cargo test -p golden --features trace`.
//! Regenerate: `DW_BLESS=1 cargo test -p golden --features trace` (review the
//! diff before committing).
//!
//! Each fixture is additionally accepted by the upstream CTSC oracle (manual,
//! not a cargo gate); see `tests/common/mod.rs` for the `validate.py`
//! invocation.

mod common;

use artifact::OtlpFormat;
use common::{compare_or_bless, expect_panic, scenario, supervisor_fault};

// --- Operation completion dispositions ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn operation_returning_value_records_result() {
    let spans = scenario("operation_result", 0, || {
        let _ = golden::add(2, 3);
    });
    compare_or_bless("operation-result.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn operation_returning_none_records_empty() {
    let spans = scenario("operation_empty", 0, || {
        let _ = golden::first_even(vec![1, 3]);
    });
    compare_or_bless("operation-empty.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn operation_returning_unit_records_no_completion() {
    let spans = scenario("operation_unit", 0, || {
        golden::log_only("hello");
    });
    compare_or_bless("operation-unit.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn operation_returning_err_records_error_status() {
    let spans = scenario("operation_error", 0, || {
        let _ = golden::lookup(0);
    });
    compare_or_bless("operation-error.otlp.json", OtlpFormat::Json, vec![spans]);
}

// --- Dependencies ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_returning_ok_records_result() {
    let spans = scenario("dep_ok", 0, || {
        let _ = golden::to_int("2a");
    });
    compare_or_bless("dep-ok.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_returning_err_records_error() {
    let spans = scenario("dep_err", 0, || {
        let _ = golden::to_int("zz");
    });
    compare_or_bless("dep-err.otlp.json", OtlpFormat::Json, vec![spans]);
}

// --- Span structure ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn chained_operations_link_and_order_spans() {
    let spans = scenario("checkout_totals", 0, || {
        let sub = golden::subtotal(1000, 15);
        let _ = golden::with_tax(sub);
    });
    compare_or_bless("sequential.otlp.json", OtlpFormat::Json, vec![spans]);
}

// --- Values ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn watch_point_encodes_ctsc_value_edges() {
    let spans = scenario("value_edges", 0, || {
        let _ = golden::value_edges();
    });
    compare_or_bless("values.otlp.json", OtlpFormat::Json, vec![spans]);
}

// --- Faults ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn operation_panic_records_target_fault() {
    let spans = scenario("target_panic", 0, || {
        expect_panic(|| {
            let _ = golden::boom(-1);
        });
    });
    compare_or_bless("target-fault.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn dep_panic_cascades_fault_to_all_frames() {
    let spans = scenario("dep_panic_cascade", 0, || {
        expect_panic(|| {
            let _ = golden::cascade_outer(-1);
        });
    });
    compare_or_bless("fault-cascade.otlp.json", OtlpFormat::Json, vec![spans]);
}

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn supervisor_fault_records_on_scenario() {
    let spans = supervisor_fault("process_abort", 0);
    compare_or_bless("supervisor-fault.otlp.json", OtlpFormat::Json, vec![spans]);
}

// --- Serialization ---

#[test]
#[cfg_attr(not(feature = "trace"), ignore = "requires the `trace` feature")]
fn jsonl_serializes_one_capture_per_line() {
    let seq = scenario("checkout_totals", 0, || {
        let sub = golden::subtotal(1000, 15);
        let _ = golden::with_tax(sub);
    });
    let dep = scenario("hex_parse", 0, || {
        let _ = golden::to_int("2a");
    });
    compare_or_bless("streaming.otlp.jsonl", OtlpFormat::Jsonl, vec![seq, dep]);
}
