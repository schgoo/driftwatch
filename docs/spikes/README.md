# Driftwatch spikes

**Frozen feasibility artifacts.** Each subdirectory is a standalone research
scratch crate captured verbatim from the spike that produced it. They are:

- **Outside the `rust/` workspace** — not members of `rust/Cargo.toml`, not built
  or tested by CI, and not gated by any Driftwatch feature.
- **Frozen** — kept as evidence of a decision, not maintained. Do not refactor
  them to track workspace changes; when the corresponding decision is
  superseded, prune the spike rather than "fixing" it.
- **Source only** — Rust `target/` build-output dirs are git-ignored
  (`docs/spikes/**/target/`); only the crate source, `Cargo.toml`, and
  `Cargo.lock` are tracked so the pinned dependency set is reproducible.

## Spikes

- [`ra-extract/`](ra-extract/) — rust-analyzer static-resolver feasibility PoC
  (Phase R). Proves real type inference recovers type-aliased error channels
  the current build-time string pipeline cannot. Verdict **GO, with
  constraints**; see [`ra-extract/REPORT.md`](ra-extract/REPORT.md) and the
  decision record
  [`../decisions/phase-r-static-resolver.md`](../decisions/phase-r-static-resolver.md).
