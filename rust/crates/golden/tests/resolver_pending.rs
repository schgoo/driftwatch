//! Tier-2 resolver-oracle harness: the acceptance cases the resolver must fix.
//!
//! This is a standalone test binary — a separate compilation unit, hence a
//! separate link-time discovery slice — whose annotated fixtures are declared
//! **locally** so they register only here and never fold into Tier-1's
//! `tests/golden/registry.json` (even under `--all-features`). It pins the
//! resolved `registry.json` the fixtures MUST produce once the rust-analyzer
//! static resolver lands (roadmap PR-4), authored by hand BEFORE the resolver
//! exists so the oracle cannot be blessed from a live (wrong) run — it is
//! un-gameable.
//!
//! The headline value is **error-channel recovery**: an alias hides a `Result`'s
//! error from the build-time *string* pipeline (`extract::classify_return` /
//! `parse_type_ref`), and only real type inference recovers it. The cases span
//! both CTSC error-naming tiers (trace-contract producer-choice #4), the
//! foreign-error *collision* property, and the alias→primitive baseline:
//!
//! - [`charge`] → `ChargeResult<i64>` — the **semantic** case. `ChargeResult`
//!   hides the `Result` shape, so today `charge`'s result is a dangling
//!   `Named("ChargeResult")` with no error outcome. Resolved:
//!   `result: i64` + `errors: [{ name: "ChargeError" }]`. `ChargeError` is a
//!   type we own with `#[derive(Watchable)]`, so its declaration is already in
//!   `types[]` today (see the two-component note below) and the recovered error
//!   is named after the type's last path segment.
//! - [`first_byte`] → `io::Result<i64>` — the **structural / cross-crate** case.
//!   The foreign alias shows one visible generic arg, so today the op looks
//!   infallible (`result: i64`, `errors: []` — the error channel is *dropped*,
//!   not dangling). Resolved: `result: i64` + `errors: [{ name: "Error" }]` —
//!   `io::Error` is foreign and can never be `Watchable`, so the name-only
//!   last-path-segment `"Error"` is the spec-mandated result (producer-choice
//!   #4), not a weakness. It carries no `types[]` entry and no `ty`.
//! - [`parse_amount`] → `anyhow::Result<i64>` — the **foreign-error collision**
//!   case, a second, *distinct* foreign crate. Today it looks infallible exactly
//!   like `first_byte` (`result: i64`, `errors: []`). Resolved: `result: i64` +
//!   `errors: [{ name: "Error" }]` — `anyhow::Error`'s last path segment is also
//!   `Error`, so two unrelated foreign error types legitimately collapse to the
//!   *same* name. This proves producer-choice #4 applies structurally and
//!   uniformly: the resolver names by last path segment and must NOT disambiguate
//!   the collision (name-only, no `ty`, no `types[]` entry).
//! - [`elapsed`] → `Millis` — the **alias→primitive** baseline. A dangling
//!   `Named("Millis")` today; resolved to the primitive `u64`.
//!
//! The error `ty` field stays `None` throughout: v1 declares errors by name
//! only, and foreign-error name collisions are spec-sanctioned.
//!
//! ## Two components, by design
//!
//! `#[derive(Watchable)]` stamps its `TypeMeta` with the crate's
//! `CARGO_PKG_NAME` (here `golden`), *not* the annotating operation's component
//! tag. So `ChargeError` registers under a `golden` component while the ops sit
//! in `resolver.pending`; the emitted (and resolved) registry therefore has two
//! components. `charge` references the error only by name (an `ErrorOutcome`
//! carries no `TypeRef`), so this cross-component split never dangles.
//!
//! # Cases considered and dropped (empirical, not aspirational)
//!
//! - **`fmt::Result`** (would resolve to `result: ()` + a recovered `fmt::Error`).
//!   It *cannot be an annotated operation today*: `fmt::Result` is
//!   `Result<(), fmt::Error>`, whose last path segment is `Result`, so the
//!   string pipeline classifies it as `ReturnKind::Result` and the macro's `Ok`
//!   arm emits `ToValue::to_value(&())` — but there is **no `impl ToValue for ()`**
//!   in `runtime`, so the wrapper fails to compile. Covering it would need a
//!   separate `runtime`/canonicalization decision, and it adds nothing on the
//!   error axis: `fmt::Error`'s last path segment is `Error`, the same name
//!   `io::Error` and `anyhow::Error` already produce. So it is documented here
//!   rather than shipped.
//!
//! # Merge-back plan (part of the RA resolver PR's definition of done)
//!
//! This split is **temporary scaffolding**. Once the RA static resolver makes
//! these cases validate cleanly:
//!
//! 1. Delete the `#[ignore]` on [`resolver_reproduces_resolved_registry`] and
//!    make it pass (the string pipeline is replaced by the resolver).
//! 2. Fold the fixtures back into the shared `golden` `src/` library (e.g.
//!    alongside `src/aliases.rs`) so they compile into Tier-1's slice.
//! 3. Merge their expected output into the single `tests/golden/registry.json`
//!    (Tier-1) and re-bless.
//! 4. Delete this test binary and `tests/golden/resolved-registry.json`.
//!
//! Removing the `#[ignore]` and performing this merge-back is part of the RA
//! resolver PR's definition of done.

#![cfg(feature = "driftwatch")]

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

use annotations::__rt::open_span;
use annotations::{SpanName, Value, Watchable, reset, watch_operation};

// --- Tier-2 fixtures (declared locally so they stay out of Tier-1's slice) ---

/// A locally-declared `Watchable` error enum: the **semantic** recovery case.
///
/// Because we own this type and derive `Watchable`, its `#[derive(Watchable)]`
/// link-time registration declares it in the registry `types[]` today (under the
/// crate-name component `golden`, the fixed `CARGO_PKG_NAME` the derive stamps).
/// The resolver names [`charge`]'s recovered error after this type's last path
/// segment — `ChargeError` — per trace-contract producer-choice #4.
#[derive(Watchable, Debug, Clone)]
enum ChargeError {
    /// A declined charge, carrying the offending amount.
    Declined {
        /// The rejected amount, in cents.
        code: i64,
    },
}

/// A local alias for a fallible charge result: `Result<T, ChargeError>`.
type ChargeResult<T> = core::result::Result<T, ChargeError>;

/// Charges `cents`, declining a negative amount — the **semantic** (Watchable,
/// ownership-based) error-recovery case.
///
/// The `ChargeResult` alias hides the `Result` shape from the string pipeline,
/// so today `charge`'s result is a dangling `Named("ChargeResult")` with no
/// error outcome. Real inference normalizes it to `Result<i64, ChargeError>`:
/// `result: i64` + a recovered `ChargeError` outcome.
///
/// # Errors
///
/// Returns [`ChargeError::Declined`] when `cents` is negative.
#[watch_operation(component = "resolver.pending")]
fn charge(cents: i64) -> ChargeResult<i64> {
    if cents < 0 {
        Err(ChargeError::Declined { code: cents })
    } else {
        Ok(cents)
    }
}

/// Returns the first byte of `text` as an integer — the **structural /
/// cross-crate** error-recovery case.
///
/// `io::Result<i64>` is a foreign alias for `Result<i64, io::Error>`; today the
/// string pipeline sees one visible generic arg and reports the op as infallible
/// (`result: i64`, `errors: []` — the error channel is silently dropped, not
/// dangling). Real inference recovers the error. Because `io::Error` is foreign
/// it can never be `Watchable`, so the recovered outcome is name-only — its last
/// path segment `Error` (producer-choice #4), with no `types[]` entry.
///
/// # Errors
///
/// Returns an [`io::Error`] of kind [`io::ErrorKind::UnexpectedEof`] when
/// `text` is empty.
#[watch_operation(component = "resolver.pending")]
fn first_byte(text: &str) -> io::Result<i64> {
    match text.bytes().next() {
        Some(byte) => Ok(i64::from(byte)),
        None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input")),
    }
}

/// Parses `s` as an integer through `anyhow::Result` — the **foreign-error
/// collision** case, a second, distinct foreign crate.
///
/// `anyhow::Result<i64>` is a foreign alias for `Result<i64, anyhow::Error>`;
/// like [`first_byte`], today the string pipeline sees one visible generic arg
/// and reports the op as infallible (`result: i64`, `errors: []`). Real
/// inference recovers the error. `anyhow::Error` is foreign (never `Watchable`),
/// so its recovered outcome is the name-only last path segment `Error`
/// (producer-choice #4) — the *same* name `first_byte` gets from the unrelated
/// `io::Error`. The resolver must NOT disambiguate the collision.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] when `s` does not parse as an `i64`.
#[watch_operation(component = "resolver.pending")]
fn parse_amount(s: &str) -> anyhow::Result<i64> {
    Ok(s.trim().parse()?)
}

/// An undeclared local type alias: `Millis` is not a `Watchable` type, so the
/// string pipeline lowers the return type to a dangling `Named("Millis")`.
type Millis = u64;

/// Returns the elapsed tick count as a `Millis` alias — the **alias→primitive**
/// case.
///
/// Today the string pipeline cannot see through the alias and emits a dangling
/// `Named("Millis")` result; the resolver normalizes it to the primitive `u64`.
#[must_use]
#[watch_operation(component = "resolver.pending")]
fn elapsed(ticks: u64) -> Millis {
    ticks
}

// --- Local emit harness (distinct outdir/target so it never collides with the
// Tier-1 `tests/golden.rs` process even under `--all-features`) ---

/// Serializes the setup/emit critical section within this binary.
static EMIT_LOCK: Mutex<()> = Mutex::new(());

/// One-time process setup: the resolved outdir holding `registry.json`.
static SETUP: OnceLock<PathBuf> = OnceLock::new();

/// The run span's required `conformance.run.id`.
fn run_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "conformance.run.id".to_string(),
        Value::String("resolver-pending-run".to_string()),
    )])
}

/// The scenario span's `conformance.scenario.{name,index}`.
fn scenario_attributes() -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "conformance.scenario.name".to_string(),
            Value::String("resolver_pending".to_string()),
        ),
        ("conformance.scenario.index".to_string(), Value::Integer(0)),
    ])
}

/// Configure the process once (distinct `[target] name` + outdir) and return the
/// resolved outdir. Mirrors `tests/common/mod.rs` but with an isolated target so
/// the two test binaries' captures never share a `registry.json`.
fn setup() -> PathBuf {
    SETUP
        .get_or_init(|| {
            let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("resolver-pending");
            let out = tmp.join("out");
            std::fs::create_dir_all(&out).expect("create the emit outdir");
            std::fs::write(
                tmp.join("driftwatch.toml"),
                "outdir = \"out\"\n[target]\nname = \"golden-resolver-pending\"\n",
            )
            .expect("write driftwatch.toml");
            std::env::set_current_dir(&tmp).expect("set cwd to the emit tempdir");
            annotations::install();
            out
        })
        .clone()
}

/// Drive one root-close (which triggers the once-per-run registry write) on a
/// fresh thread, then read the emitted `registry.json` back.
fn emit_registry_bytes() -> String {
    let _guard = EMIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let out = setup();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            reset();
            let run = open_span(SpanName::Run, run_attributes());
            let scenario = open_span(SpanName::Scenario, scenario_attributes());
            // Invoke the fixtures so the module is unambiguously linked and its
            // link-time registrations are retained.
            let _ = charge(10);
            let _ = first_byte("hi");
            let _ = elapsed(5);
            let _ = parse_amount("7");
            drop(scenario);
            drop(run);
        });
    });
    std::fs::read_to_string(out.join("registry.json")).expect("read the emitted registry.json")
}

/// Resolve `tests/golden/<name>` at the repository root, mirroring
/// `tests/common/mod.rs::golden_path` (compile-time-absolute `CARGO_MANIFEST_DIR`
/// so the test-side `set_current_dir` never affects it).
fn golden_path(name: &str) -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.push("../../../tests/golden");
    root.push(name);
    root
}

/// Read and parse the hand-authored resolved target.
fn resolved_document() -> (String, contract::RegistryDocument) {
    let bytes = std::fs::read_to_string(golden_path("resolved-registry.json"))
        .expect("read resolved-registry.json");
    let doc = contract::RegistryDocument::parse(&bytes).expect("resolved-registry.json parses");
    (bytes, doc)
}

// --- Tests ---

/// Asserts `op` resolved to an `i64` result carrying exactly one name-only
/// foreign/semantic error (`ty` absent, per v1 producer-choice #4).
fn assert_i64_with_named_error(op: &contract::Operation, error_name: &str) {
    assert_eq!(
        op.outcomes.result,
        Some(contract::TypeRef::Primitive {
            name: contract::Primitive::I64
        }),
        "`{}` result is `i64`",
        op.name
    );
    assert_eq!(op.outcomes.errors.len(), 1, "`{}` has one error", op.name);
    assert_eq!(op.outcomes.errors[0].name, error_name);
    assert!(
        op.outcomes.errors[0].ty.is_none(),
        "the recovered error is name-only (v1)"
    );
}

/// Guards the integrity of the hand-authored oracle: it must be a valid,
/// zero-violation registry with the resolved shapes the RA resolver will
/// produce — the semantic (`ChargeError`), structural (`io::Error`), collision
/// (`anyhow::Error`), and alias→primitive (`Millis`) recoveries — with nothing
/// dangling. Runs today.
#[test]
fn resolved_target_validates() {
    let (_bytes, doc) = resolved_document();

    let violations = contract::validate(&doc);
    assert!(
        violations.is_empty(),
        "resolved-registry.json must validate cleanly, got: {violations:?}"
    );

    // Two components: the crate-name `golden` component the Watchable derive
    // stamps `ChargeError` into, and `resolver.pending` holding the operations.
    let ids: Vec<&str> = doc.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        ["golden", "resolver.pending"],
        "components sorted by id"
    );

    // `ChargeError` is declared as a tagged_union with the `Declined { code: i64 }`
    // variant — byte-exact from the `#[derive(Watchable)]` link-time registration.
    let golden = &doc.components[0];
    assert!(golden.operations.is_empty(), "the type-only component");
    assert_eq!(golden.types.len(), 1);
    match &golden.types[0] {
        contract::NamedType::TaggedUnion { name, variants, .. } => {
            assert_eq!(name, "ChargeError");
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name, "Declined");
            let payload = variants[0]
                .payload
                .as_ref()
                .expect("Declined carries a record payload");
            let contract::TypeRef::Record { fields } = payload else {
                panic!("Declined payload must be an inline record, got {payload:?}");
            };
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "code");
            assert_eq!(
                fields[0].ty,
                contract::TypeRef::Primitive {
                    name: contract::Primitive::I64
                }
            );
        }
        other @ contract::NamedType::Record { .. } => {
            panic!("ChargeError must be a tagged_union, got {other:?}")
        }
    }

    // The four operations, sorted by name.
    let pending = &doc.components[1];
    assert_eq!(pending.id, "resolver.pending");
    let names: Vec<&str> = pending
        .operations
        .iter()
        .map(|op| op.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["charge", "elapsed", "first_byte", "parse_amount"],
        "sorted by name"
    );

    // `charge`: the `ChargeResult` alias resolved to `Result<i64, ChargeError>`
    // — the semantic (Watchable, ownership-based) error recovery. The error is
    // named after the type's last path segment; `ty` stays absent (v1).
    assert_i64_with_named_error(&pending.operations[0], "ChargeError");

    // `elapsed`: the local alias lowered to the `u64` primitive, no errors.
    let elapsed = &pending.operations[1];
    assert_eq!(
        elapsed.outcomes.result,
        Some(contract::TypeRef::Primitive {
            name: contract::Primitive::U64
        }),
        "the `Millis` alias resolves to `u64`"
    );
    assert!(
        elapsed.outcomes.errors.is_empty(),
        "`elapsed` has no error channel"
    );

    // `first_byte`: the foreign `io::Error` recovered as the name-only `Error`
    // (producer-choice #4) — the structural / cross-crate case, no `types[]`.
    assert_i64_with_named_error(&pending.operations[2], "Error");

    // `parse_amount`: a second, distinct foreign crate whose `anyhow::Error`
    // collapses to the *same* name-only `Error` — the collision producer-choice
    // #4 sanctions. The resolver must not disambiguate it from `io::Error`.
    assert_i64_with_named_error(&pending.operations[3], "Error");
}

/// The live-emit acceptance test. RED today — the string pipeline cannot produce
/// the resolved output — so it is ignored; the RA resolver PR's definition of
/// done is to delete the `#[ignore]` and make this byte-compare pass.
#[test]
#[ignore = "enabled by the RA static resolver (roadmap PR-4)"]
fn resolver_reproduces_resolved_registry() {
    let emitted = emit_registry_bytes();
    let (expected, _doc) = resolved_document();
    assert_eq!(
        emitted, expected,
        "the resolver-emitted registry.json must match the hand-authored oracle"
    );
}
