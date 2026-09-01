# Driftwatch — migration roadmap

> **Status:** planning. No Rust crates exist yet. This roadmap is the durable,
> cross-session source of truth for the SpecGate → Driftwatch migration.

## What Driftwatch is

Driftwatch captures a **contract** (static API shape, from the link-time
registry) plus **behavioral traces** (dynamic, from annotated code execution)
out of a codebase, and **diffs two captures** to catch version-to-version
drift. The honest guarantee is **change detection, not correctness**: "these
observable behaviors changed between A and B — did you mean to?" This is
AI-tamper-resistant because the oracle is *another version's own behavior* —
there is no hand-authored assertion for an AI to weaken.

Origin: a clean-room reboot of [SpecGate](../../specgate), keeping only its
**extraction** half (annotations → traces + registry → contract) and dropping
the TDD/matcher half (hand-authored `expected:` cases, operator catalog, spec
test codegen).

### Concepts / vocabulary

- **Contract** — the static, verifiable, diagnosable API shape: operations,
  types, inputs/outputs. Extracted from the link-time registry (`discover`).
  *Static, complete, deterministic, input-free.*
- **Trace** — the dynamic event stream an operation emits when driven with
  inputs. *Requires execution, coverage-limited, needs canonicalization.*
- **Extraction / snapshot** — one code version's `{contract, traces}`.
- **Compare** — diff two extractions → contract-diff (breaking-change) +
  trace-diff (behavioral divergence).

## Comparison modes (CLI, modeled on `cargo evaluate`)

- Primitive: `driftwatch compare <A> <B>` — diff two already-extracted artifacts
  (no build, no git).
- `driftwatch snapshot <path> -o run.dw` — extract one version → artifact.
- `--mode full` — snapshot once (or vs a stored baseline).
- `--mode diff --base main` — snapshot base + head, compare.
- `--mode pr --pr <url>` — as diff, scoped to PR-changed files.

## Artifact format

Driftwatch emits **CTSC 0.1** (see `docs/trace-contract.md`). One extraction
produces two CTSC artifacts:

- **Registry** (`ctsc.registry`) — the normalized contract (registry → typed
  operations), from `discover`.
- **Trace** — OTLP `TracesData` (`.otlp.json`/`.otlp.jsonl`), from the runtime
  buffer: `conformance.run`→`scenario`→`operation` spans with input attributes,
  observation events, and one completion each.

Two producing mechanisms (SpecGate's `discover` / `run_spec`): static registry →
Registry, dynamic buffer → Trace. Two diffs under a **CTSC comparison policy**
(default CTSC Strict): structural registry diff (contract) and operation/event/
value trace diff. The contract-diff **contextualizes** the trace-diff.

## Trace emission contract — CTSC producer profile

Driftwatch emits **CTSC 0.1** (Conformance Trace Semantic Conventions):
OTLP-based conformance **Trace** + language-neutral **Registry** + **Comparison**
policy. [`docs/trace-contract.md`](trace-contract.md) is Driftwatch's producer
profile (annotation→CTSC mapping + producer choices); CTSC itself is normative.
Adopting CTSC resolves every prior open decision (see **Ratified decisions**):
sum/enum → single-key kvlist tagged union (D4) and goldens → CTSC `.otlp.json`
corpus (D5); non-finite floats wire as OTLP `doubleValue` strings (CTSC §8.4);
sets emit in stable order (set semantics only under Linked); comparison is
**CTSC Strict**. This realigns the artifact format (#11 → emit CTSC OTLP, not a
bespoke binary), the golden corpus (#5 → CTSC corpus), and the registry/discover
+ diff slices onto CTSC.

## Salvage map (from SpecGate)

| Take (▪ lift · ⚠ refactor) | Leave behind |
|---|---|
| ▪ `specgate-runtime` → `runtime` (buffer, `Value`, `SpecEvent`, registry) | matcher + operator catalog (`$gt`, `$unordered`, …); the lossy `serde_json` record mode |
| ▪ `specgate-annotations(-macros)` | `expected:` cases, narrative/level/provenance |
| ⚠ `specgate-types` → `contract` (strip `cases`) | self-host / conformance authored ledgers |
| ⚠ harness driver slice: binding-resolve + codegen + build+run+collect + `discover` | `validate` case-runnability, `extract --cases` |
| ▪ C# `SpecGate.Weaver` + `SpecGate.Runtime` | spec-case codegen, the matcher tail of `run_spec` |
| ▪ the #36 trace goldens + encoder edge tests (the extraction TCB) | |

Trust anchor carried over: the SpecGate #36 work (exact-trace goldens, `Value`
encoding edge tables, canonicalization findings) — see **Invariants**.
Canonicalization is **load-bearing**: identical behavior must yield identical
extractions or the diff reports engine artifacts as false drift.

## Work breakdown (target ≤500 net LoC each)

> Tracked as GitHub issues. Each item is referenced by its issue number (`#n`);
> ordering is by phase, not by id. Decisions (D*) and invariants (F*) are
> reference matter, not work items — see their own sections.

Type: ▪ mechanical lift · ⚠ real refactor · ✚ net-new.

### Phase 0 — preserve & scaffold
- **SpecGate #36 preservation** *(SpecGate repo — not tracked as a driftwatch issue)* — commit the source we lift from.
- **#1** — driftwatch scaffold: `rust/` workspace, `rust/crates/`, justfile
  (build/test/clippy/fmt + **`evaluate`**), Rust-only CI, licenses, `.gitignore`,
  toolchain, README — **plus the agent loop** (`AGENTS.md` +
  `.github/agents/*.agent.md` observe-plan-act-verify orchestrator +
  planner/implementer/reviewer specialists + `.github/skills/`) and the
  **planning-doc tree** (`docs/README.md`, `docs/agentic-loop.md`,
  `docs/digests/{human,llm}.md`, `docs/references.md`) — modeled on
  `~/repos/dcf`. Wire **`cargo evaluate`** into the `check` recipe + CI as a
  harness gate (with a starter `evaluate.toml`). ✚ ~450

### Phase 1 — emitter (extraction TCB)

All emitter work implements a clause of [`docs/trace-contract.md`](trace-contract.md).

- **#2** ✅ — `runtime` pt1: `Value` (strict structural `Eq`/`Ord`, `Debug`) + `TraceEvent` + `ToValue`; #36 encoder edge tests. Dropped SpecGate's loose equality and hand-written serde (serialization deferred to the artifact-format item). ▪
- **#3** — `runtime` pt2: buffer, `emit_*`/`take_traces`/`reset`, registry (`OpMeta`/`TypeMeta`/discovery), `SpecEvent`. **No serialization** (deferred to #11). ▪ ~400
- **#4** ✅ — core annotations: `#[watch_operation]` + `#[watch_input]` + `watch_point!` + facade re-exports; per-parameter inputs, field-mutation echo, `$result` via `ReturnEmit`. Macro surface committed as `watch_*` (D1). ▪
- **#29** — `#[derive(Watchable)]`: structural struct/enum emission (includes the Watchable merge). Unblocks the struct/enum outcome clauses. ▪
- **#30** ✅ — `#[watch_dep]` dependency-boundary tracer: per-arg inputs + real-call `.response`/`.error`, optional `?`, `compile_error!` on unsupported shapes. Observation-only (substitution dropped → #27). ▪

- **Emission contract ratified** ✅ — `docs/trace-contract.md` adopts CTSC 0.1 as Driftwatch's producer profile; D1–D5 resolved. Prereq for the emission items and the golden corpus.
- **#37** — CTSC completion events: emit operation completion as `conformance.result`/`.empty`/`.error`/`.fault`, and dependencies as **nested `conformance.operation` spans** (own inputs + completion). Value carries via one `ReturnEmit` ladder (L1→L4-opaque). Retires the `Map` `{Ok|Err}`/`{Some|None}` tags and the bespoke `d.response`/`d.error` dep vocabulary. ⚠ ~350
- **#38** — dependency initializer shapes: `?` ✅; `.await`/async-dep support (needs an async operation + test executor); combinators stay `compile_error!`. ✚
- **#39** — panic disposition: on a caught panic emit `conformance.fault` (+ partial trace, `ERROR` status), no result. ✚
- **#40** — input surface: capture `&mut` non-receiver params as inputs (pre-call value) — supersedes #4's `&mut` exclusion; hidden inputs stay annotator-captured via `watch_point!`. Update `operation_params` goldens. ⚠
- **#42** — parallel branches: emit `conformance.parallel` with **unordered** child branches from concurrently-executing operations/dependencies. Defines cross-thread `SpanContext` propagation with **globally-unique `span_id`s** (lane-encoded: per-thread lane ∥ per-capture counter) so merged multi-thread spans keep unambiguous `parent_span_id` linkage; single-threaded captures stay byte-deterministic. Deferred from #37a, whose span substrate is single-threaded. Comparator pairs branches by identity, not order. ✚ ~350 · **load-bearing** (id-minting scheme)

- **#5** — the trace **golden corpus** as CTSC `.otlp.json` fixtures (per D5) covering every profile clause; re-author the SpecGate `mock_*` goldens as real-dependency (nested-operation) observation. Reuse SpecGate's CTSC corpus + `validate.py`. The emission trust anchor + the ongoing TDD spec (new clause → golden → implement). ▪ ~400
- **#24** — **mutation testing** capstone: `cargo-mutants` scoped to the emitter
  TCB (`runtime` + `annotations(-macros)`), run against the native encoder tests
  + trace goldens only (non-circular oracle), survivors triaged to zero. The
  "measure the trust anchor" gate — SpecGate #36 Rung 4 analog. Depends on #5
  (the goldens are the kill oracle). ✚

### Phase 1.5 — artifact format (reprioritized; load-bearing)
- **#11** — **CTSC OTLP emitter**: serialize the runtime buffer to CTSC Trace
  (`.otlp.json`/`.otlp.jsonl`) and `Value`→OTLP `AnyValue` per profile §Values
  (`u64`→decimal string, set→stable-ordered array, non-finite float→OTLP
  `doubleValue` string with NaN payloads normalized). Foundational — every
  persistence path (record, collect, snapshot, compare) depends on it. ✚
  load-bearing

### Phase 2 — contract
- **#6** — `contract`: lift types, strip `cases`, keep name/types/operations/binding + validation; `.spec.yaml`→`.contract.yaml`. ⚠ ~450

### Phase 3 — extraction driver (drop the matcher)
- **#7** — binding resolution (drop matcher bits). ▪ ~350
- **#8** — runner codegen: drive ops → emit traces (artifact format, no `expected:`/matcher). ⚠ ~450
- **#9** — build+run+collect: invoke cargo/dotnet, capture CTSC traces (`run_spec` front half minus match tail). ⚠ ~400
- **#10** — contract extraction: `discover` slice (registry → normalized schema). ▪ ~350


### Phase 5 — diff (new)
- **#12** — contract-diff: structural diff + breaking-change classification + report. ✚ ~450
- **#13** — trace-diff: CTSC comparison (Strict) — pair operations, diff events/values, first-divergence report. ✚ ~450

### Phase 6 — CLI (new)
- **#14** — CLI skeleton + `snapshot` (extract → artifact). ✚ ~300
- **#15** — `compare A B` + `--format human/json/llm`. ✚ ~350
- **#16** — `--mode full/diff/pr`, `--base`, `--pr` orchestration. ✚ ~400

### Phase 7 — C# backend (parallelizable after Phase 1)
- **#17** — lift `SpecGate.Runtime` (C# emitter → CTSC OTLP). ▪ ~400
- **#18** — lift `SpecGate.Weaver` (IL weave). ▪ ~450
- **#19** — C# trace goldens, driven & case-aligned with Rust. ✚ ~300
- **#20** — cross-language compare wiring (drive both, diff). ✚ ~300

## Dependencies & critical path

Linear within phases: 1→2→3→4→5. Phase 2 independent after scaffold. Phase 7
branches after Phase 1.

**Fastest path to a real demo (contract-diff — cheap, un-gameable, no trace
driver or C# needed):** #1 → #6 → #10 → #12 → #14 → #15.

Mutation testing (#24) gates the emitter TCB once its goldens exist; it is an
on-demand / periodic run (`just mutants`), not a per-PR gate, because mutation
runs are slow.

## Ratified decisions

Emission-shape / canonicalization decisions (**D1–D5**) are **resolved** by
adopting CTSC 0.1; see [`docs/trace-contract.md`](trace-contract.md).

- **D1** — macro naming: `watch_*` (final).
- **D2** — operation/dependency unification: a dependency is a nested
  `conformance.operation`.
- **D3** — panic disposition: `conformance.fault`.
- **D4** — sum/enum → single-key kvlist tagged union (`Value::Variant`).
- **D5** — goldens → CTSC `.otlp.json` corpus.

Also settled:

- Artifact format: **CTSC OTLP JSON** (`.otlp.json`/`.otlp.jsonl`) — owned by #11.
- Ship contract-diff (#12) before trace-diff (#13): metadata drift is the cheap,
  always-available, coverage-independent first product.

Changing any of these requires human ratification and a corpus update
(`trace-contract.md` §Change control).

## Invariants (the emitter must uphold)

Carried from the SpecGate #36 trust anchor; canonicalization is load-bearing.

- **F1** — set ≠ list only under Linked; a set wires as a stable (sorted-order)
  array at Trace Core.
- **F2** — non-finite floats wire as OTLP `doubleValue` strings (`"NaN"` /
  `"Infinity"` / `"-Infinity"`, CTSC §8.4); the runtime normalizes NaN payload
  bits at the OTLP boundary so bitwise-distinct NaNs do not surface as false
  drift.
- **F3** — strict `AnyValue` structural equality: `Value` is equal iff same
  variant and contents (SpecGate's loose Int==Float / List==Set equality dropped).

## Related SpecGate issues (context)

- #36 — trace-engine trust anchor + trace contract (the extraction TCB; source
  of the goldens/encoder tests/canonicalization findings).
- #26 — differentiation from TDD / spec-as-data (refactor-oracle = the compare
  idea).
- #30 — proptest inputs → trace diff (the trace-coverage feeder; north-star).