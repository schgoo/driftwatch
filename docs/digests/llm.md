# LLM digest — Driftwatch

Operational facts and decision boundaries for coding agents. Update only when
durable facts change.

## What Driftwatch is

Captures a **contract** (static API shape, from the link-time registry) plus
**behavioral traces** (dynamic, from annotated code) and **diffs two captures**
to detect version-to-version drift. Guarantee: **change detection, not
correctness**. AI-tamper-resistant because the oracle is another version's own
behavior — no hand-authored assertion to weaken.

## Current state

- **Phase: planning.** Only `AGENTS.md` and `docs/` exist. No Rust crates yet.
- Migration plan: `docs/roadmap.md` (20 PRs, phases 0–7).
- Origin: clean-room reboot of SpecGate; lift the extraction half, drop the
  TDD/matcher half.

## Invariants

- Trace goldens assert on derived `Debug`, never `Value` `PartialEq` (loose:
  Int==Float, List==Set) and never the serde wire form (Set serializes like
  List; NaN not JSON-representable). These are the F1/F2/F3 canonicalization
  findings carried from SpecGate #36.
- Contract extraction is static/complete/deterministic; trace extraction is
  dynamic/coverage-limited and must be canonicalized before diffing.
- One artifact per code version: `{contract, traces[keyed]}`. The trace key
  (op + canonicalized inputs) aligns runs across versions.

## Decision boundaries (need a human owner)

- Artifact format/extension (`.dw`? JSON vs keyed-JSONL).
- Trace canonicalization rules.
- CLI surface (`snapshot` / `compare` / `--mode full|diff|pr`).
- Macro naming (`spec_*` vs `dw_*`).

## Harness

Every PR passes `cargo test` + `cargo evaluate` + `cargo clippy -D warnings` +
`cargo fmt --check`. `cargo evaluate` is a merge gate, not advisory.
