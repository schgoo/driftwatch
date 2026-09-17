//! End-to-end proof of the live auto-emit path (`--features driftwatch`).
//!
//! One integration test binary, one `#[test]`: the writer is process-global, so
//! a single test avoids parallel siblings racing the shared writer and the
//! process working directory. It drives N concurrent threads through a
//! run/scenario frame and asserts the appended `trace.otlp.jsonl` has exactly N
//! independently-parseable lines (interleaving would corrupt at least one), one
//! per scenario name — proving atomic, locked per-line writes structurally.
#![cfg(feature = "driftwatch")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;

use annotations::__rt::open_span;
use annotations::{SpanName, Value, reset, watch_operation};

/// A minimal annotated fixture op exercised inside each scenario.
#[watch_operation(component = "emit.test")]
fn ping(x: i64) -> i64 {
    x
}

fn run_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "conformance.run.id".to_string(),
        Value::String("emit-run".to_string()),
    )])
}

fn scenario_attributes(name: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "conformance.scenario.name".to_string(),
            Value::String(name.to_string()),
        ),
        ("conformance.scenario.index".to_string(), Value::Integer(0)),
    ])
}

/// Pull `conformance.scenario.name` out of a parsed `TracesData` line.
fn scenario_name(line_json: &serde_json::Value) -> String {
    let spans = line_json["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .expect("scopeSpans[0].spans is an array");
    for span in spans {
        for attr in span["attributes"].as_array().into_iter().flatten() {
            if attr["key"] == serde_json::json!("conformance.scenario.name") {
                return attr["value"]["stringValue"]
                    .as_str()
                    .expect("scenario name is a stringValue")
                    .to_string();
            }
        }
    }
    panic!("no conformance.scenario.name attribute in line");
}

#[test]
fn auto_emit_writes_non_interleaved_jsonl() {
    const N: usize = 16;

    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("emit8");
    fs::create_dir_all(&tmp).expect("create tmp dir");
    // No `set_var` (unsafe in edition 2024): configure via a written file plus a
    // safe `set_current_dir`, so config resolution walks up to this file.
    fs::write(
        tmp.join("driftwatch.toml"),
        "outdir = \"out\"\n[target]\nname = \"golden-corpus\"\n",
    )
    .expect("write driftwatch.toml");
    std::env::set_current_dir(&tmp).expect("set cwd to tmp");
    let out = tmp.join("out").join("trace.otlp.jsonl");
    let _ = fs::remove_file(&out);

    annotations::install();

    let barrier = Arc::new(Barrier::new(N));
    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            // Gate all threads so their appends genuinely contend for the lock.
            barrier.wait();
            reset();
            let run = open_span(SpanName::Run, run_attributes());
            let scenario = open_span(SpanName::Scenario, scenario_attributes(&format!("t{i}")));
            let _ = ping(i64::try_from(i).expect("small index"));
            drop(scenario);
            drop(run); // root close → one appended line
        }));
    }
    for handle in handles {
        handle.join().expect("scenario thread");
    }

    let contents = fs::read_to_string(&out).expect("read trace.otlp.jsonl");
    assert!(contents.ends_with('\n'), "each line is newline-terminated");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), N, "exactly one line per root close");

    let mut names = Vec::with_capacity(N);
    for line in &lines {
        // Interleaving would corrupt at least one line's JSON.
        let value: serde_json::Value =
            serde_json::from_str(line).expect("each line parses as standalone JSON");
        assert!(
            value.get("resourceSpans").is_some(),
            "each line is a TracesData document"
        );
        names.push(scenario_name(&value));
    }
    names.sort();
    let mut expected: Vec<String> = (0..N).map(|i| format!("t{i}")).collect();
    expected.sort();
    assert_eq!(
        names, expected,
        "each scenario name appears exactly once (no line lost to interleaving)"
    );
}
