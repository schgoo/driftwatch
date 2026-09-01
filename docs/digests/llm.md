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

- **Phase: 1 (emitter).** `rust/` workspace + crate scaffold exist (`runtime`, `annotations(-macros)`, `contract`, `extract`, `diff`, `cli`); the emitter crates are implemented through the annotation surface (#1-#4, #29-#30 merged). CTSC emission migration (#37 onward) is the active work.
- Migration plan: `docs/roadmap.md` (issue-numbered work items, phases 0–7).
- Origin: clean-room reboot of SpecGate; lift the extraction half, drop the
  TDD/matcher half.

## Invariants

- `Value` has strict, structural `Eq`/`Ord` (#2): equal iff same variant and
  contents; `eq` is `cmp(..) == Equal`. SpecGate's loose equality (Int==Float,
  List==Set) was removed, so `==` is faithful (resolves the old F3, kept as CTSC
  strict `AnyValue` equality). Under CTSC: a `Set` wires as a stable-ordered array
  (set≠list only under Linked, F1); non-finite floats wire as OTLP `doubleValue` strings
  (`"NaN"`/`"Infinity"`/`"-Infinity"`, CTSC §8.4; NaN payloads normalized, F2). Values serialize as OTLP `AnyValue`.
- Contract extraction is static/complete/deterministic; trace extraction is
  dynamic/coverage-limited and must be canonicalized before diffing.
- One extraction per code version → two CTSC artifacts: a Registry (contract) and
  an OTLP Trace. CTSC Strict pairs operations by position; input-keyed matching is
  a possible future custom policy.
- The synthetic return event is named `$result`, keeping the `$` sigil (decided
  in #31). Params emit as `op.<name>`; the return emits a bare `$result`, so the
  sigil reserves a namespace a user field/param named `result` cannot shadow,
  and marks the event as compiler-synthesized. It is baked into the frozen
  `runtime` crate (`return_emit.rs`) and echoed by `annotations-macros`
  (`operation.rs`); do NOT rename it — that would edit frozen `runtime` and
  churn trace canonicalization (the event-name vocabulary the trace-diff keys
  on).

## Decision boundaries (need a human owner)

- Artifact format: **CTSC OTLP JSON** (`.otlp.json`/`.otlp.jsonl`) — resolved via CTSC adoption.
- Trace emission contract: **CTSC 0.1 producer profile** (docs/trace-contract.md); D1-D5 resolved. Observable shape + canonicalization owned there.
- CLI surface (`snapshot` / `compare` / `--mode full|diff|pr`).

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
