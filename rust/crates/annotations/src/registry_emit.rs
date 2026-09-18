//! The feature-gated once-per-run CTSC `registry.json` writer.
//!
//! On the first capture of a `driftwatch` run, [`emit_registry`] materializes a
//! [`contract::RegistryDocument`], serializes it as pretty JSON plus a trailing
//! newline — the committed-artifact convention, round-trippable through
//! `RegistryDocument::parse` — and writes `<outdir>/registry.json` alongside the
//! trace.
//!
//! There are two ways the document is derived:
//!
//! - **Link-time (default).** Derive from the discovery registry
//!   (`runtime::DRIFTWATCH_OPS` / `runtime::DRIFTWATCH_TYPES`) via
//!   [`extract::derive`] — the alias-blind string pipeline. This is the only
//!   path unless the `resolve` feature is compiled in *and* a source is set.
//! - **Static resolver (opt-in, `resolve` feature).** When built with the
//!   `resolve` feature *and* `driftwatch.toml` sets a `[resolver] source`, the
//!   emitter resolves that target crate with rust-analyzer
//!   ([`resolver::resolve`]) — recovering alias-hidden error channels the string
//!   path drops — and assembles the registry from the resolved IR via
//!   [`contract::assemble`]. With no `[resolver] source` configured the emitter
//!   keeps the link-time path, so the `resolve` feature is purely additive: it
//!   never changes default behavior (the Tier-1 golden stays byte-identical even
//!   under `--all-features`).
//!
//! It shares the trace-emit gate: [`crate::emit::init_state`] only calls this
//! once it has resolved a `[target] name` (which is also the `registryId`) and
//! created the outdir, so when the trace path soft-skips the registry does too.
//! Any derivation/resolution/serialization/write failure here **warns to stderr
//! and skips** the registry — it never aborts the process and never breaks trace
//! emission. An empty registry (no operations *and* no types) is skipped
//! entirely so the `extract` `"default"`-component fallback never surfaces in a
//! live artifact.

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

/// Route the once-per-run write: the static-resolver path when the `resolve`
/// feature is compiled *and* a `[resolver] source` is configured, otherwise the
/// link-time derivation. Keeping the resolver path gated on an explicit source
/// (not on the feature alone) is what makes `resolve` purely additive.
fn write_registry(config: &artifact::CaptureConfig, target_name: &str) {
    #[cfg(feature = "resolve")]
    if let Some(source) = config.resolver_source.as_deref() {
        write_resolved_registry(config, target_name, source);
        return;
    }
    write_derived_registry(config, target_name);
}

/// The link-time derivation path: skip an empty registry, otherwise
/// derive → serialize → write, warning to stderr on any error.
fn write_derived_registry(config: &artifact::CaptureConfig, target_name: &str) {
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
    write_document(config, &document);
}

/// The static-resolver path (`resolve` feature): resolve `source` with
/// rust-analyzer, then assemble the registry from the alias-resolved IR. A
/// [`resolver::ResolveError`] warns to stderr and skips the registry — it never
/// aborts the run or breaks trace emission (mirroring the write failure policy).
#[cfg(feature = "resolve")]
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

    // Empty-registry guard, applied to the resolved IR (mirrors the link-time
    // path): a target with no operations and no types emits no document.
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
