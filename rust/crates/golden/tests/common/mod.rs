//! The golden harness: the live-emit infrastructure that drives each fixture
//! through the real root-close sink (the `driftwatch` feature), plus the
//! compare-or-bless comparators.
//!
//! Every fixture runs on a **fresh spawned thread** (so the per-thread
//! id/tick/trace counters always start at zero → fully deterministic ids,
//! independent of cargo's worker-thread reuse) and its span tree is exported by
//! the installed sink as one compact JSONL line appended to a process-global
//! `trace.otlp.jsonl`. A process-global [`EMIT_LOCK`] brackets each test's
//! truncate → emit → read of that shared file so parallel `#[test]`s never race.
//!
//! The committed goldens stay human-inspectable: a `.otlp.json` golden is the
//! sink's compact line losslessly pretty-printed (the emitter builds its JSON
//! from `serde_json`'s default `BTreeMap`, so keys are canonically sorted and all
//! numbers are bare small ints or CTSC strings — `to_string_pretty` on the
//! parsed line reproduces the committed bytes exactly). The `.otlp.jsonl`
//! golden is compared raw/compact, line for line.
//!
//! # External acceptance (manual, not a cargo gate)
//!
//! Each `.otlp.json` — and each line of the `.jsonl` — is additionally accepted
//! by the upstream CTSC `validate.py trace` oracle.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

use annotations::__rt::{open_span, push_fault, set_status};
use annotations::{SpanName, SpanStatus, Value, reset};

/// Serializes each test's truncate → emit → read of the process-global trace
/// file, and guards the one-time config/cwd/install setup below. Cargo runs
/// `#[test]`s in parallel on a shared pool, so without this lock two tests could
/// interleave writes to (or reads of) the single shared `trace.otlp.jsonl`.
static EMIT_LOCK: Mutex<()> = Mutex::new(());

/// One-time process setup, run inside the [`EMIT_LOCK`] critical section on the
/// first emit: it caches the resolved `trace.otlp.jsonl` path.
static SETUP: OnceLock<PathBuf> = OnceLock::new();

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

/// Install the live emit sink. Split behind a `cfg` so `common` still compiles
/// without the feature (the goldens are then `#[ignore]`d and never reach here).
#[cfg(feature = "driftwatch")]
fn install_sink() {
    annotations::install();
}

#[cfg(not(feature = "driftwatch"))]
fn install_sink() {
    unreachable!("golden emit requires the `driftwatch` feature");
}

/// Configure the process once and return the resolved trace-file path.
///
/// No `env::set_var` (it is `unsafe` in edition 2024): the capture is configured
/// via a written `driftwatch.toml` plus a safe `set_current_dir`, so the sink's
/// config resolution walks up to it. `golden_path` uses the compile-time
/// `CARGO_MANIFEST_DIR`, so changing cwd does not break golden-file resolution.
fn setup() -> PathBuf {
    SETUP
        .get_or_init(|| {
            let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden-emit");
            let out = tmp.join("out");
            std::fs::create_dir_all(&out).expect("create the emit outdir");
            std::fs::write(
                tmp.join("driftwatch.toml"),
                "outdir = \"out\"\n[target]\nname = \"golden-corpus\"\n",
            )
            .expect("write driftwatch.toml");
            std::env::set_current_dir(&tmp).expect("set cwd to the emit tempdir");
            install_sink();
            out.join("trace.otlp.jsonl")
        })
        .clone()
}

/// Open a `conformance.run` + `conformance.scenario` frame (the extraction
/// driver's eventual job), run `body` inside the scenario, then close the frame.
/// Closing the root span (the run) triggers the sink to append one JSONL line.
///
/// It must NOT call `take_spans()` — the sink already drains the buffer on root
/// close. Distinct `reset()`s re-zero the id/tick counters and mint a fresh
/// (monotonic) trace id, so back-to-back scenarios on one thread get distinct
/// trace ids (`..01`, `..02`, …) while everything else stays deterministic.
pub fn run_scenario<F: FnOnce()>(name: &str, index: i64, body: F) {
    reset();
    let run = open_span(SpanName::Run, run_attributes());
    let scenario = open_span(SpanName::Scenario, scenario_attributes(name, index));
    body();
    drop(scenario);
    drop(run);
}

/// Drive a full emit cycle: hold [`EMIT_LOCK`], ensure setup, truncate the
/// shared trace file, run `body` on a FRESH spawned thread (so the per-thread
/// counters start at zero → deterministic ids), then read the file back and
/// return its contents. `body` performs one or more [`run_scenario`] calls; each
/// root close appends one line.
///
/// The lock is captured *only* to bracket truncate → emit → read; it is
/// released when this function returns (the returned `String` owns the bytes),
/// so the caller's `assert_eq!`/bless runs **outside** the lock. Asserting while
/// holding `EMIT_LOCK` would poison it on a golden mismatch and cascade the
/// failure into every parallel test — capture bytes under the lock, compare
/// after it.
pub fn emit<F: FnOnce() + Send>(body: F) -> String {
    let _guard = EMIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let path = setup();
    // Truncate so only this cycle's lines are present. The sink holds its own
    // append handle; O_APPEND / FILE_APPEND_DATA recomputes EOF per write, so
    // the next appended line lands at offset 0 after this truncation.
    std::fs::write(&path, b"").expect("truncate the shared trace file");
    std::thread::scope(|scope| {
        scope.spawn(body);
    });
    std::fs::read_to_string(&path).expect("read the emitted trace file")
}

/// Run one fixture scenario on a fresh thread and return the emitted file
/// contents (exactly one compact JSONL line).
pub fn emit_scenario<F: FnOnce() + Send>(name: &str, index: i64, body: F) -> String {
    emit(move || run_scenario(name, index, body))
}

/// A run/scenario frame that records a harness-emitted **supervisor** fault on
/// the scenario span (no annotation produces one) and marks the scenario Error.
/// Returns the emitted file contents (one compact JSONL line).
pub fn supervisor_fault(name: &str, index: i64) -> String {
    emit_scenario(name, index, || {
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

/// Resolve `tests/golden/<name>`. By default the shared cross-language corpus
/// lives at the repository root (`rust/crates/golden` → three parents → repo
/// root); `DW_GOLDEN_DIR` overrides the directory with an absolute path so the
/// suite still finds the corpus when run from a sandboxed copy of the workspace
/// (e.g. under `cargo gamma`, whose scratch tree does not include repo-root
/// siblings of the cargo workspace). This uses the compile-time-absolute
/// `CARGO_MANIFEST_DIR`, so the test-side `set_current_dir` does not affect it.
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

/// Byte-compare `rendered` against the on-disk golden `name`, or regenerate it
/// under `DW_BLESS=1`.
fn write_or_compare(name: &str, rendered: &str) {
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

/// Compare a single-capture `.otlp.json` golden against the sink's one JSONL
/// line, losslessly pretty-printed via a `serde_json` round-trip (canonical
/// `BTreeMap` key order + bare-int/CTSC-string numbers make this byte-exact), or
/// regenerate it under `DW_BLESS=1`.
pub fn compare_or_bless_json(name: &str, emitted: &str) {
    let line = emitted.trim_end_matches('\n');
    assert!(
        !line.contains('\n'),
        "a single-capture .otlp.json golden must come from exactly one JSONL line"
    );
    let value: serde_json::Value =
        serde_json::from_str(line).expect("the emitted JSONL line parses as JSON");
    let mut pretty = serde_json::to_string_pretty(&value)
        .expect("re-serializing a parsed serde_json::Value is infallible");
    // Pretty JSON has no trailing newline; keep goldens newline-terminated.
    pretty.push('\n');
    write_or_compare(name, &pretty);
}

/// Compare the multi-capture streaming `.otlp.jsonl` golden against the sink's
/// raw compact output (one `\n`-terminated line per capture — jsonl stays
/// raw/compact, it is never pretty-printed), or regenerate it under `DW_BLESS=1`.
pub fn compare_or_bless_jsonl(name: &str, emitted: &str) {
    write_or_compare(name, emitted);
}

/// Drive one trivial capture so the sink's once-per-run init writes
/// `registry.json`, then read those bytes back from the resolved outdir.
///
/// The registry is derived from the *link-time* discovery registry (every
/// annotated fixture in this crate), not from what the probe scenario runs, and
/// it is written once per process — so any emit cycle materializes it. It lives
/// beside `trace.otlp.jsonl` (which the trace goldens truncate per cycle) and is
/// read from its own stable path, never from the trace file.
pub fn emit_registry_bytes() -> String {
    // One trivial root-close triggers `init_state`, which writes the registry.
    let _ = emit_scenario("registry_probe", 0, || {});
    let path = {
        let _guard = EMIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        setup()
            .parent()
            .expect("the trace path has a parent outdir")
            .join("registry.json")
    };
    std::fs::read_to_string(&path).expect("read the emitted registry.json")
}

/// Byte-compare the emitted `registry.json` against its on-disk golden `name`
/// (the emitter already writes pretty JSON + trailing newline, the committed
/// on-disk form), or regenerate it under `DW_BLESS=1`.
pub fn compare_or_bless_registry(name: &str, emitted: &str) {
    write_or_compare(name, emitted);
}
