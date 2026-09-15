//! The golden harness: a fixed [`Resource`], the trace-id pin, run/scenario
//! framing, and the compare-or-bless comparator.
//!
//! # External acceptance (manual, not a cargo gate)
//!
//! Each `.otlp.json` — and each line of the `.jsonl` — is additionally accepted
//! by the upstream CTSC `validate.py trace` oracle.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use annotations::__rt::{open_span, push_fault, set_status};
use annotations::{Span, SpanName, SpanStatus, Value, reset, take_spans};
use artifact::{OtlpFormat, Resource, TraceCapture};
use std::collections::BTreeMap;

/// The fixed CTSC resource paired with every golden capture (tool + target
/// identity); `conformance.version` is injected by the emitter.
pub fn resource() -> Resource {
    Resource::new("driftwatch", "0.1.0", "golden-corpus", "rust")
}

/// The run span's required `conformance.run.id`.
fn run_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "conformance.run.id".to_string(),
        Value::String("golden-run".to_string()),
    )])
}

/// The scenario span's `conformance.scenario.name` + `conformance.scenario.index`.
fn scenario_attributes(name: &str, index: i64) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "conformance.scenario.name".to_string(),
            Value::String(name.to_string()),
        ),
        (
            "conformance.scenario.index".to_string(),
            Value::Integer(index),
        ),
    ])
}

/// Reset the runtime buffer, open a `conformance.run` + `conformance.scenario`
/// frame (the extraction driver's eventual job), run `body` inside the scenario,
/// close the frame, and return the drained spans (run → scenario → operations).
pub fn scenario<F: FnOnce()>(name: &str, index: i64, body: F) -> Vec<Span> {
    reset();
    let run = open_span(SpanName::Run, run_attributes());
    let scenario = open_span(SpanName::Scenario, scenario_attributes(name, index));
    body();
    drop(scenario);
    drop(run);
    take_spans()
}

/// A run/scenario frame that records a harness-emitted **supervisor** fault on
/// the scenario span (no annotation produces one) and marks the scenario Error.
pub fn supervisor_fault(name: &str, index: i64) -> Vec<Span> {
    scenario(name, index, || {
        push_fault(
            "process_exit",
            "supervisor",
            "target process aborted".to_string(),
        );
        set_status(SpanStatus::Error);
    })
}

/// Run `f`, swallowing the default panic hook's stderr noise, and assert it
/// unwound (proving the fault path re-propagates rather than being swallowed).
pub fn expect_panic(f: impl FnOnce()) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    outcome.expect_err("the fixture must panic to record a fault");
}

/// A fixed nonzero trace id. The comparator ignores the correlator, but the
/// on-disk bytes must be stable, so the harness overwrites the nondeterministic
/// per-thread trace id on every drained span. Distinct `index` values keep the
/// two `.jsonl` captures from colliding on `(traceId, spanId)`.
fn pinned_trace_id(index: u8) -> [u8; 16] {
    let mut id = [0x11_u8; 16];
    id[15] = index + 1;
    id
}

fn pin(mut spans: Vec<Span>, index: u8) -> Vec<Span> {
    let tid = pinned_trace_id(index);
    for span in &mut spans {
        span.trace_id = tid;
    }
    spans
}

/// Resolve `tests/golden/<name>`. By default the shared cross-language corpus
/// lives at the repository root (`rust/crates/golden` → three parents → repo
/// root); `DW_GOLDEN_DIR` overrides the directory with an absolute path so the
/// suite still finds the corpus when run from a sandboxed copy of the workspace
/// (e.g. under `cargo gamma`, whose scratch tree does not include repo-root
/// siblings of the cargo workspace).
fn golden_path(name: &str) -> PathBuf {
    let mut path = if let Some(dir) = std::env::var_os("DW_GOLDEN_DIR") {
        PathBuf::from(dir)
    } else {
        let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        root.push("../../../tests/golden");
        root
    };
    path.push(name);
    path
}

/// Serialize `captures` (each pinned with a distinct trace id) in `format`, then
/// byte-compare against the on-disk golden — or regenerate it under `DW_BLESS`.
///
/// A `.otlp.json` fixture passes a single capture; the streaming `.otlp.jsonl`
/// passes several, each rendered as one `\n`-terminated `TracesData` line.
pub fn compare_or_bless(name: &str, format: OtlpFormat, captures: Vec<Vec<Span>>) {
    let mut rendered = String::new();
    for (index, spans) in captures.into_iter().enumerate() {
        let capture = TraceCapture {
            resource: resource(),
            spans: pin(spans, u8::try_from(index).expect("few captures per golden")),
        };
        rendered.push_str(&capture.to_otlp(format));
    }
    // Pretty JSON has no trailing newline; keep goldens newline-terminated.
    if format == OtlpFormat::Json && !rendered.ends_with('\n') {
        rendered.push('\n');
    }

    let path = golden_path(name);
    if std::env::var_os("DW_BLESS").is_some() {
        std::fs::write(&path, rendered.as_bytes())
            .unwrap_or_else(|e| panic!("writing golden {}: {e}", path.display()));
    } else {
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "reading golden {} (regenerate with DW_BLESS=1): {e}",
                path.display()
            )
        });
        assert_eq!(
            rendered, expected,
            "golden {name} drifted from the on-disk bytes; regenerate with DW_BLESS=1"
        );
    }
}
