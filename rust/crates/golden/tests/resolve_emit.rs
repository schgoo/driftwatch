//! Resolver-emit path proof (Phase R, R-d): drive the live registry-emit gate
//! with the `resolve` feature and a `[resolver] source` configured, and show the
//! emitted `registry.json` is produced by rust-analyzer — recovering an aliased
//! error channel the build-time string pipeline drops.
//!
//! The fixture operation `first_byte` returns a `std::io::Result<i64>` alias.
//! The string pipeline (`extract::derive`) sees one visible generic arg and
//! reports the op as **infallible** (`errors: []`). Real type inference recovers
//! the foreign `io::Error` as the name-only `Error` (producer-choice #4). This
//! test asserts the *resolved* shape reaches the on-disk `registry.json`, proving
//! the additive resolver-emit path is wired end to end.
//!
//! Gated on `resolve`: without the feature there is no resolver dependency and
//! nothing to prove. This does NOT touch the `#[ignore]`d acceptance oracle in
//! `resolver_pending.rs` (that byte-compare + merge-back is R-e).

#![cfg(feature = "resolve")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

use annotations::__rt::open_span;
use annotations::{SpanName, Value, reset};

/// Serializes the setup/emit critical section within this binary.
static EMIT_LOCK: Mutex<()> = Mutex::new(());

/// One-time process setup: the resolved outdir holding `registry.json`.
static SETUP: OnceLock<PathBuf> = OnceLock::new();

/// The fixture crate rust-analyzer resolves, addressed by its compile-time
/// absolute path (independent of the test-side `set_current_dir`).
fn fixture_source() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("tests");
    dir.push("fixtures");
    dir.push("resolve_target");
    dir
}

/// The run span's required `conformance.run.id`.
fn run_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "conformance.run.id".to_string(),
        Value::String("resolve-emit-run".to_string()),
    )])
}

/// The scenario span's `conformance.scenario.{name,index}`.
fn scenario_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "conformance.scenario.name".to_string(),
            Value::String("resolve_emit".to_string()),
        ),
        ("conformance.scenario.index".to_string(), Value::Integer(0)),
    ])
}

/// Configure the process once — an isolated outdir plus a `[resolver] source`
/// pointing at the fixture crate — and return the resolved outdir. The source is
/// written as a TOML *literal* (single-quoted) string so Windows backslashes are
/// not interpreted as escapes.
fn setup() -> PathBuf {
    SETUP
        .get_or_init(|| {
            let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("resolve-emit");
            let out = tmp.join("out");
            std::fs::create_dir_all(&out).expect("create the emit outdir");
            let source = fixture_source();
            let config = format!(
                "outdir = \"out\"\n[target]\nname = \"resolve-demo\"\n[resolver]\nsource = '{}'\n",
                source.display()
            );
            std::fs::write(tmp.join("driftwatch.toml"), config).expect("write driftwatch.toml");
            std::env::set_current_dir(&tmp).expect("set cwd to the emit tempdir");
            annotations::install();
            out
        })
        .clone()
}

/// Drive one root-close (which triggers the once-per-run registry write via the
/// resolver) on a fresh thread, then read the emitted `registry.json` back.
fn emit_registry_bytes() -> String {
    let _guard = EMIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let out = setup();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            reset();
            let run = open_span(SpanName::Run, run_attributes());
            let scenario = open_span(SpanName::Scenario, scenario_attributes());
            drop(scenario);
            drop(run);
        });
    });
    std::fs::read_to_string(out.join("registry.json")).expect("read the emitted registry.json")
}

/// The headline proof: the resolver-emit path writes a `registry.json` whose
/// `first_byte` operation recovers the alias-hidden `io::Error` channel — an
/// `i64` result plus a single name-only `Error` outcome — which the string
/// pipeline drops entirely.
#[test]
fn resolver_emit_recovers_aliased_error_channel() {
    let emitted = emit_registry_bytes();
    let doc = contract::RegistryDocument::parse(&emitted).expect("registry.json parses");

    // The document validates cleanly through the contract oracle.
    let violations = contract::validate(&doc);
    assert!(
        violations.is_empty(),
        "resolver-emitted registry.json must validate cleanly, got: {violations:?}"
    );

    // The single `resolve.demo` component holds the resolved `first_byte`.
    let component = doc
        .components
        .iter()
        .find(|c| c.id == "resolve.demo")
        .expect("the `resolve.demo` component is present");
    let op = component
        .operations
        .iter()
        .find(|op| op.name == "first_byte")
        .expect("the `first_byte` operation is present");

    // The `io::Result<i64>` alias resolved to an `i64` result...
    assert_eq!(
        op.outcomes.result,
        Some(contract::TypeRef::Primitive {
            name: contract::Primitive::I64
        }),
        "`first_byte` result is `i64`"
    );
    // ...plus the recovered, name-only foreign error the string path drops.
    assert_eq!(
        op.outcomes.errors.len(),
        1,
        "the alias-hidden `io::Error` channel is recovered"
    );
    assert_eq!(op.outcomes.errors[0].name, "Error");
    assert!(
        op.outcomes.errors[0].ty.is_none(),
        "the recovered foreign error is name-only (producer-choice #4)"
    );
}
