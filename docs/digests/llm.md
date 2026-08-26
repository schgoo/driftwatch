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

- `Value` has strict, structural `Eq`/`Ord` (PR2 #2): equal iff same variant and
  contents; `eq` is `cmp(..) == Equal`. SpecGate's loose equality (Int==Float,
  List==Set) was removed, so `==` is a faithful comparison here (this resolves
  the old F3). The serde wire form is still lossy — a `Set` serializes like a
  `List`, and `NaN`/`Inf` are not JSON-representable (the standing F1/F2) — so it
  is for persistence, not comparison.
- Contract extraction is static/complete/deterministic; trace extraction is
  dynamic/coverage-limited and must be canonicalized before diffing.
- One artifact per code version: `{contract, traces[keyed]}`. The trace key
  (op + canonicalized inputs) aligns runs across versions.
- Synthetic return events are named exactly `$result`; the `$` reserves the
  compiler-synthesized namespace. Renaming it requires an explicit human-owned
  trace-contract decision.

## Decision boundaries (need a human owner)

- Artifact format/extension (`.dw`? JSON vs keyed-JSONL).
- Trace canonicalization rules.
- CLI surface (`snapshot` / `compare` / `--mode full|diff|pr`).
- Macro naming (`spec_*` vs `dw_*`).

## File organization

Implementation lives in named, concern-scoped sibling files; crate roots
(`lib.rs`/`main.rs`) and `mod.rs` only declare modules and re-export
(`mod value; pub use value::Value;`), one public path per item. `cargo
evaluate`'s `m_balanced_modules` judges the module namespace, not physical file
size, so a god-file `lib.rs` will NOT be flagged by the harness — this is a
human-enforced convention (see `AGENTS.md`).

## Harness

Every PR passes `cargo test` + `cargo evaluate` + `cargo clippy -D warnings` +
`cargo fmt --check`. `cargo evaluate` is a merge gate, not advisory.
