//! The feature-gated once-per-run CTSC `registry.json` writer.
//!
//! On the first capture of a `driftwatch` run, [`emit_registry`] materializes a
//! [`contract::RegistryDocument`], serializes it as pretty JSON plus a trailing
//! newline — the committed-artifact convention, round-trippable through
//! `RegistryDocument::parse` — and writes `<outdir>/registry.json` alongside the
//! trace.
//!
//! The document is produced by the **static resolver**: the emitter resolves the
//! target crate with rust-analyzer ([`resolver::resolve`]) — recovering
//! alias-hidden error channels a string pipeline would drop — and assembles the
//! registry from the resolved IR via [`contract::assemble`]. The source crate is
//! the runtime `CARGO_MANIFEST_DIR` of the process under capture by default; a
//! `[resolver] source` in `driftwatch.toml`
//! ([`artifact::CaptureConfig::resolver_source`]) is an explicit override.
//!
//! It shares the trace-emit gate: [`crate::emit::init_state`] only calls this
//! once it has resolved a `[target] name` (which is also the `registryId`) and
//! created the outdir, so when the trace path soft-skips the registry does too.
//! Any resolution/serialization/write failure here **warns to stderr and
//! skips** the registry — it never aborts the process and never breaks trace
//! emission. An empty registry (no operations *and* no types) is skipped
//! entirely so no synthetic component ever surfaces in a live artifact.

use std::fs;
use std::path::PathBuf;
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

/// Resolve the source crate and write the registry once. The source is the
/// explicit `[resolver] source` override when configured, otherwise the runtime
/// `CARGO_MANIFEST_DIR` of the process under capture (never the compile-time
/// `env!` of *this* crate, and never the cwd — which the harness relocates to a
/// tempdir with no `Cargo.toml`). With no source available, the registry is
/// skipped (warn, never abort).
fn write_registry(config: &artifact::CaptureConfig, target_name: &str) {
    let Some(source) = resolver_source(config) else {
        eprintln!(
            "driftwatch: no resolver source (set `[resolver] source` or run under cargo so \
             CARGO_MANIFEST_DIR is set); skipping registry"
        );
        return;
    };
    write_resolved_registry(config, target_name, &source);
}

/// The crate the resolver aims at: the explicit `[resolver] source` override, or
/// the runtime `CARGO_MANIFEST_DIR` of the process under capture.
fn resolver_source(config: &artifact::CaptureConfig) -> Option<PathBuf> {
    config
        .resolver_source
        .clone()
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from))
}

/// Resolve `source` with rust-analyzer, then assemble the registry from the
/// alias-resolved IR. A [`resolver::ResolveError`] warns to stderr and skips the
/// registry — it never aborts the run or breaks trace emission (mirroring the
/// write failure policy).
fn write_resolved_registry(
    config: &artifact::CaptureConfig,
    target_name: &str,
    source: &std::path::Path,
) {
    let target = resolver::ResolveTarget::new(source);
    let resolved = match resolver::resolve(&target) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!(
                "driftwatch: cannot resolve {} with rust-analyzer: {err}; skipping registry",
                source.display()
            );
            return;
        }
    };

    // Empty-registry guard: a target with no operations and no types emits no
    // document, so no synthetic component ever surfaces in a live artifact.
    if resolved.operations.is_empty() && resolved.types.is_empty() {
        return;
    }

    let document = contract::assemble(
        &resolved.operations,
        &resolved.types,
        target_name.to_string(),
        config.resolve_registry_version(),
    );
    write_document(config, &document);
}

/// Serialize `document` as pretty JSON + trailing newline (the committed
/// artifact form) and write it to `<outdir>/registry.json`, warning to stderr on
/// a serialization or write failure.
fn write_document(config: &artifact::CaptureConfig, document: &contract::RegistryDocument) {
    let json = match serde_json::to_string_pretty(document) {
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
