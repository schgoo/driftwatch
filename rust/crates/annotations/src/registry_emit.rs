//! The feature-gated once-per-run CTSC `registry.json` writer.
//!
//! On the first capture of a `driftwatch` run, [`emit_registry`] derives a
//! [`contract::RegistryDocument`] from the link-time discovery registry
//! (`runtime::DRIFTWATCH_OPS` / `runtime::DRIFTWATCH_TYPES`) via
//! [`extract::derive`], serializes it as pretty JSON plus a trailing newline —
//! the committed-artifact convention, round-trippable through
//! `RegistryDocument::parse` — and writes `<outdir>/registry.json` alongside the
//! trace.
//!
//! It shares the trace-emit gate: [`crate::emit::init_state`] only calls this
//! once it has resolved a `[target] name` (which is also the `registryId`) and
//! created the outdir, so when the trace path soft-skips the registry does too.
//! Any derivation/serialization/write failure here **warns to stderr and skips**
//! the registry — it never aborts the process and never breaks trace emission.
//! An empty registry (no operations *and* no types) is skipped entirely so the
//! `extract` `"default"`-component fallback never surfaces in a live artifact.

use std::fs;
use std::sync::OnceLock;

/// Guards the once-per-run registry write. The first successful reach here
/// materializes `registry.json`; later captures in the same run are no-ops.
// Process-global once-guard for the single registry write, mirroring the
// ambient emit state in `emit.rs`: the annotation surface has no place to
// thread an explicit handle, and correctness rests on the single-runtime
// invariant. It carries no data, only the "already ran" edge.
#[cfg_attr(false, allow(evaluate::m_avoid_statics))]
static REGISTRY_WRITTEN: OnceLock<()> = OnceLock::new();

/// Derive and write `registry.json` into `config.outdir` exactly once per run,
/// stamping `registry_id` with `target_name`. Warns and skips on any failure.
pub(crate) fn emit_registry(config: &artifact::CaptureConfig, target_name: &str) {
    REGISTRY_WRITTEN.get_or_init(|| write_registry(config, target_name));
}

/// The one-shot body run under [`REGISTRY_WRITTEN`]: skip an empty registry,
/// otherwise derive → serialize → write, warning to stderr on any error.
fn write_registry(config: &artifact::CaptureConfig, target_name: &str) {
    let ops = &runtime::DRIFTWATCH_OPS;
    let types = &runtime::DRIFTWATCH_TYPES;

    // Empty-registry guard: no operations and no types → do not emit a synthetic
    // `"default"`-component document (roadmap #10b, decision 4).
    if ops.is_empty() && types.is_empty() {
        return;
    }

    let identity = extract::RegistryIdentity {
        registry_id: target_name.to_string(),
        version: config.resolve_registry_version(),
    };
    let document = extract::derive(ops, types, identity);

    let json = match serde_json::to_string_pretty(&document) {
        Ok(mut json) => {
            json.push('\n');
            json
        }
        Err(err) => {
            eprintln!("driftwatch: cannot serialize registry.json: {err}; skipping registry");
            return;
        }
    };

    let path = config.outdir.join("registry.json");
    if let Err(err) = fs::write(&path, json.as_bytes()) {
        eprintln!(
            "driftwatch: cannot write {}: {err}; skipping registry",
            path.display()
        );
    }
}
