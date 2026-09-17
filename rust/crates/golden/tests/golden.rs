//! The golden harness tests: one focused scenario per test case, each named
//! `<subject>_<condition>_<result>`. Every test drives one reusable fixture on a
//! fresh thread, frames it with a run/scenario span, and lets the **live emit
//! sink** export it as one JSONL line to `trace.otlp.jsonl`; the test reads that
//! line back and byte-compares it — pretty-printed for a `.otlp.json` golden,
//! raw for the streaming `.otlp.jsonl` — against its on-disk golden under
//! `tests/golden/`.
//!
//! Compare: `cargo test -p golden --features driftwatch`.
//! Regenerate: `DW_BLESS=1 cargo test -p golden --features driftwatch` (review
//! the diff before committing).
//!
//! Each fixture is additionally accepted by the upstream CTSC oracle (manual,
//! not a cargo gate); see `tests/common/mod.rs` for the `validate.py`
//! invocation.

mod common;

use common::{
    compare_or_bless_json, compare_or_bless_jsonl, compare_or_bless_registry, emit,
    emit_registry_bytes, emit_scenario, expect_panic, run_scenario, supervisor_fault,
};

// --- Operation completion dispositions ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn operation_returning_value_records_result() {
    let emitted = emit_scenario("operation_result", 0, || {
        let _ = golden::add(2, 3);
    });
    compare_or_bless_json("operation-result.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn operation_returning_none_records_empty() {
    let emitted = emit_scenario("operation_empty", 0, || {
        let _ = golden::first_even(vec![1, 3]);
    });
    compare_or_bless_json("operation-empty.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn operation_returning_unit_records_no_completion() {
    let emitted = emit_scenario("operation_unit", 0, || {
        golden::log_only("hello");
    });
    compare_or_bless_json("operation-unit.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn operation_returning_err_records_error_status() {
    let emitted = emit_scenario("operation_error", 0, || {
        let _ = golden::lookup(0);
    });
    compare_or_bless_json("operation-error.otlp.json", &emitted);
}

// --- Dependencies ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn dep_returning_ok_records_result() {
    let emitted = emit_scenario("dep_ok", 0, || {
        let _ = golden::to_int("2a");
    });
    compare_or_bless_json("dep-ok.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn dep_returning_err_records_error() {
    let emitted = emit_scenario("dep_err", 0, || {
        let _ = golden::to_int("zz");
    });
    compare_or_bless_json("dep-err.otlp.json", &emitted);
}

// --- Span structure ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn chained_operations_link_and_order_spans() {
    let emitted = emit_scenario("checkout_totals", 0, || {
        let sub = golden::subtotal(1000, 15);
        let _ = golden::with_tax(sub);
    });
    compare_or_bless_json("sequential.otlp.json", &emitted);
}

// --- Values ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn watch_point_encodes_ctsc_value_edges() {
    let emitted = emit_scenario("value_edges", 0, || {
        let _ = golden::value_edges();
    });
    compare_or_bless_json("values.otlp.json", &emitted);
}

// --- Faults ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn operation_panic_records_target_fault() {
    let emitted = emit_scenario("target_panic", 0, || {
        expect_panic(|| {
            let _ = golden::boom(-1);
        });
    });
    compare_or_bless_json("target-fault.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn dep_panic_cascades_fault_to_all_frames() {
    let emitted = emit_scenario("dep_panic_cascade", 0, || {
        expect_panic(|| {
            let _ = golden::cascade_outer(-1);
        });
    });
    compare_or_bless_json("fault-cascade.otlp.json", &emitted);
}

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn supervisor_fault_records_on_scenario() {
    let emitted = supervisor_fault("process_abort", 0);
    compare_or_bless_json("supervisor-fault.otlp.json", &emitted);
}

// --- Serialization ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn jsonl_serializes_one_capture_per_line() {
    // Two scenarios on ONE thread, no truncation between them: sequential
    // `reset()`s mint distinct trace ids (`..01`, `..02`), so the sink appends
    // two compact lines — the multi-capture JSONL framing under test.
    let emitted = emit(|| {
        run_scenario("checkout_totals", 0, || {
            let sub = golden::subtotal(1000, 15);
            let _ = golden::with_tax(sub);
        });
        run_scenario("hex_parse", 0, || {
            let _ = golden::to_int("2a");
        });
    });
    compare_or_bless_jsonl("streaming.otlp.jsonl", &emitted);
}

// --- Registry (contract) ---

#[test]
#[cfg_attr(
    not(feature = "driftwatch"),
    ignore = "requires the `driftwatch` feature"
)]
fn registry_emit_matches_golden_and_validates() {
    // The live sink writes `registry.json` once per run, derived from every
    // annotated fixture in this crate. Assert the emitted bytes are byte-exact
    // against the committed golden AND that they parse + validate cleanly (zero
    // §7/§8/§10 violations) through the `contract` oracle.
    let emitted = emit_registry_bytes();
    compare_or_bless_registry("registry.json", &emitted);

    let doc = contract::RegistryDocument::parse(&emitted).expect("registry.json parses");
    let violations = contract::validate(&doc);
    assert!(
        violations.is_empty(),
        "emitted registry.json must validate cleanly, got: {violations:?}"
    );
}
