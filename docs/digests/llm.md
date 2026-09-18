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

- **Phase: 1 (emitter).** `rust/` workspace + crate scaffold exist (`runtime`, `annotations(-macros)`, `artifact`, `contract`, `extract`, `diff`, `cli`); the emitter crates are implemented through the annotation surface (#1-#4, #29-#30 merged), CTSC emission (#37-#40) is complete, and the CTSC OTLP artifact emitter (#11) serializes a capture to `.otlp.json`/`.otlp.jsonl`. The trace golden corpus (#5) is the trust anchor, and mutation testing (#24, `just mutants`) validated it at 0 survivors.
- Migration plan: `docs/roadmap.md` (issue-numbered work items, phases 0–7).
- **Phase R (static resolver): planned/started.** Replaces the build-time
  *string* type-resolution front-half of **#10** (registry emission —
  `extract::derive`'s `classify_return`/`parse_type_ref`) with an
  rust-analyzer resolver so type-aliased error channels are recovered; #10's
  output contract stays byte-reproducible and downstream Phase 5/6 (#12–#16)
  consumers are untouched. Four
  gates ratified (placement after Phase 3 / same-feature dep-gating, RA is the
  default path when on / `derive()` redefined over resolved `contract` types /
  errors stay name-only, foreign-error canonicalization deferred). Feasibility
  spike tracked at `docs/spikes/ra-extract/` (GO, with constraints); decision
  record `docs/decisions/phase-r-static-resolver.md`; PRs R-a…R-e. Acceptance
  gate: the red-by-design `#[ignore]` on `resolver_reproduces_resolved_registry`
  (`rust/crates/golden/tests/resolver_pending.rs` +
  `tests/golden/resolved-registry.json`) flips green.
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
- Operation completion is emitted as CTSC span events —
  `conformance.result` (unwrapped success), `conformance.empty` (deliberate
  absence, e.g. `None`), or `conformance.error` (structural decomposition:
  `error.name` = variant tag or the last path segment of `E`; `error.value` =
  payload or Display string) — completion is real span events, not a synthetic
  result placeholder. Inputs are
  one `conformance.operation.inputs` kvlist attribute keyed by bare identifier;
  a `watch_dep!(…)` is a nested `conformance.operation` span; `watch_point!` and
  field mutations emit `conformance.observation` events. The flat `WatchEvent` /
  `emit_*` / `take_events` path is retired.

## Decision boundaries (need a human owner)

- Artifact format: **CTSC OTLP JSON** (`.otlp.json`/`.otlp.jsonl`) — resolved via CTSC adoption; **implemented** in the `artifact` crate (#11). The `artifact` crate renders a `TraceCapture` (caller-supplied `Resource` + `Vec<runtime::Span>`) to a CTSC `TracesData`, emitting `conformance.version` = `"0.2.0"` and, on the panic-fault path, `conformance.fault.type` = `"unexpected"`.
- Trace emission contract: **CTSC 0.2 producer profile** (docs/trace-contract.md); D1-D5 resolved. Observable shape + canonicalization owned there.
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

`just mutants` runs a HYBRID mutation gate that measures the emitter trust
anchor: `cargo-gamma` (config `rust/gamma.toml`) mutates the `runtime` emitter
and `cargo-mutants` (config `rust/.cargo/mutants.toml`) mutates the
`annotations(-macros)` proc-macros gamma cannot reach; both require the runtime
encoder tests + trace goldens to kill every viable mutant. A survivor is a gap
in the goldens — add a golden. On-demand, not a per-PR gate (slow). Baseline
(#24): runtime = 206 mutants → 206 killed, 0 survived, 0 uncovered (100.0%),
`registry.rs` excluded pending its contract oracle (#6/#10).
