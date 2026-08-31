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
Adopting CTSC resolves every prior open decision: **D4** (nested sum/enum →
single-key kvlist tagged union) and **D5** (goldens → CTSC `.otlp.json` corpus);
non-finite floats project to a registry tagged union; sets emit in stable order
(Linked for set semantics); comparison is **CTSC Strict**. This realigns the
artifact format (PR11 → emit CTSC OTLP, not a bespoke binary), the golden corpus
(PR5 → CTSC corpus), and the registry/discover + diff slices onto CTSC.

## Salvage map (from SpecGate)

| Take (▪ lift · ⚠ refactor) | Leave behind |
|---|---|
| ▪ `specgate-runtime` → `runtime` (buffer, `Value`, `SpecEvent`, registry) | matcher + operator catalog (`$gt`, `$unordered`, …); the lossy `serde_json` record mode |
| ▪ `specgate-annotations(-macros)` | `expected:` cases, narrative/level/provenance |
| ⚠ `specgate-types` → `contract` (strip `cases`) | self-host / conformance authored ledgers |
| ⚠ harness driver slice: binding-resolve + codegen + build+run+collect + `discover` | `validate` case-runnability, `extract --cases` |
| ▪ C# `SpecGate.Weaver` + `SpecGate.Runtime` | spec-case codegen, the matcher tail of `run_spec` |
| ▪ the #36 trace goldens + encoder edge tests (the extraction TCB) | |

Trust anchor carried over: the #36 work (exact-trace goldens, `Value` encoding
edge tables, canonicalization findings). Under CTSC: set≠list only under Linked
(F1), non-finite floats projected to a tagged union (F2), strict `AnyValue`
equality (F3). Canonicalization is **load-bearing**:
identical behavior must yield identical extractions or the diff reports engine
artifacts as false drift.

## PR breakdown (target ≤500 net LoC each)

> Tracked as GitHub issues **[#1–#20](https://github.com/schgoo/driftwatch/issues)** —
> PR_k_ = issue #_k_.

Type: ▪ mechanical lift · ⚠ real refactor · ✚ net-new.

### Phase 0 — preserve & scaffold
- **PR0** — commit the SpecGate #36 work to its branch (preserve the source we lift from). *(separate repo)*
- **PR1** — driftwatch scaffold: `rust/` workspace, `rust/crates/`, justfile
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

- **PR2** ✅ — `runtime` pt1: `Value` (strict structural `Eq`/`Ord`, `Debug`) + `TraceEvent` + `ToValue`; #36 encoder edge tests. Dropped SpecGate's loose equality and hand-written serde (serialization deferred to the artifact-format PR). ▪
- **PR3** — `runtime` pt2: buffer, `emit_*`/`take_traces`/`reset`, registry (`OpMeta`/`TypeMeta`/discovery), `SpecEvent`. **No serialization** (deferred to PR11). ▪ ~400
- **PR4 core** ✅ *(gh #28)* — `#[watch_operation]` + `#[watch_input]` + `watch_point!` + facade re-exports; per-parameter inputs, field-mutation echo, `$result` via `ReturnEmit`. Macro surface committed as `watch_*` (contract **D1**). ▪
- **PR4b** *(gh #29 / PR #33, open)* — `#[derive(Watchable)]`: structural struct/enum emission. Unblocks the struct/enum outcome clauses. ▪
- **PR4c** ✅ *(gh #30 / PR #35)* — `#[watch_dep]` dependency-boundary tracer: per-arg inputs + real-call `.response`/`.error`, optional `?`, `compile_error!` on unsupported shapes. Observation-only (substitution dropped → #27). ▪

- **F0 — ratify the emission contract** ✅ — `docs/trace-contract.md` adopts CTSC 0.1 as Driftwatch's producer profile; D1–D5 resolved (naming, op/dep unification via nested operations, panic→`conformance.fault`, sum/enum→tagged union, goldens→CTSC OTLP corpus). Prereq for the E-items and the golden corpus.
- **E1 — CTSC completion events** *(profile Events / D2+D3)*: emit operation completion as `conformance.result`/`.empty`/`.error`/`.fault`, and dependencies as **nested `conformance.operation` spans** (own inputs + completion). Value carries via one `ReturnEmit` ladder (L1→L4-opaque). Retires the `Map` `{Ok|Err}`/`{Some|None}` tags and the bespoke `d.response`/`d.error` dep vocabulary. ⚠ ~350
- **E2 — merge PR4b** (Watchable derive) — prerequisite for struct/enum goldens.
- **E3 — dependency initializer shapes** *(contract §4.3)*: `?` ✅; `.await`/async-dep support (needs an async operation + test executor); combinators stay `compile_error!`. ✚
- **E4 — panic disposition** *(profile D3)*: on a caught panic emit `conformance.fault` (+ partial trace, `ERROR` status), no result.
- **E5 — input surface** *(contract §2)*: capture `&mut` non-receiver params as
  inputs (pre-call value) — supersedes PR4-core's `&mut` exclusion; hidden
  inputs stay annotator-captured via `watch_point!`. Update `operation_params`
  goldens.

- **PR5** *(gh #5)* — the trace **golden corpus** as CTSC `.otlp.json` fixtures (per **D5**) covering every profile clause; re-author the SpecGate `mock_*` goldens as real-dependency (nested-operation) observation. Reuse SpecGate's CTSC corpus + `validate.py`. The emission trust anchor + the ongoing TDD spec (new clause → golden → implement). ▪ ~400
- **PR5.5** *(gh #24)* — **mutation testing** capstone: `cargo-mutants` scoped to
  the emitter TCB (`runtime` + `annotations(-macros)`), run against the native
  encoder tests + trace goldens only (non-circular oracle), survivors triaged to
  zero. The "measure the trust anchor" gate — SpecGate #36 Rung 4 analog. Depends
  on PR5 (the goldens are the kill oracle). ✚

### Phase 1.5 — artifact format (reprioritized; load-bearing)
- **PR11** *(moved earlier from Phase 4)* — **CTSC OTLP emitter**: serialize the
  runtime buffer to CTSC Trace (`.otlp.json`/`.otlp.jsonl`) and `Value`→OTLP
  `AnyValue` per profile §Values (`u64`→decimal string, non-finite→tagged union,
  set→stable-ordered array). JSON is faithful (non-finite floats are projected,
  not raw). Foundational — every persistence path (record, collect, snapshot,
  compare) depends on it. ✚ load-bearing

### Phase 2 — contract
- **PR6** — `contract`: lift types, strip `cases`, keep name/types/operations/binding + validation; `.spec.yaml`→`.contract.yaml`. ⚠ ~450

### Phase 3 — extraction driver (drop the matcher)
- **PR7** — binding resolution (drop matcher bits). ▪ ~350
- **PR8** — runner codegen: drive ops → emit traces (artifact format, no `expected:`/matcher). ⚠ ~450
- **PR9** — build+run+collect: invoke cargo/dotnet, capture CTSC traces (`run_spec` front half minus match tail). ⚠ ~400
- **PR10** — contract extraction: `discover` slice (registry → normalized schema). ▪ ~350


### Phase 5 — diff (new)
- **PR12** — contract-diff: structural diff + breaking-change classification + report. ✚ ~450
- **PR13** — trace-diff: CTSC comparison (Strict) — pair operations, diff events/values, first-divergence report. ✚ ~450

### Phase 6 — CLI (new)
- **PR14** — CLI skeleton + `snapshot` (extract → artifact). ✚ ~300
- **PR15** — `compare A B` + `--format human/json/llm`. ✚ ~350
- **PR16** — `--mode full/diff/pr`, `--base`, `--pr` orchestration. ✚ ~400

### Phase 7 — C# backend (parallelizable after Phase 1)
- **PR17** — lift `SpecGate.Runtime` (C# emitter → CTSC OTLP). ▪ ~400
- **PR18** — lift `SpecGate.Weaver` (IL weave). ▪ ~450
- **PR19** — C# trace goldens, driven & case-aligned with Rust. ✚ ~300
- **PR20** — cross-language compare wiring (drive both, diff). ✚ ~300

## Dependencies & critical path

Linear within phases: 1→2→3→4→5. Phase 2 independent after scaffold. Phase 7
branches after Phase 1.

**Fastest path to a real demo (contract-diff — cheap, un-gameable, no trace
driver or C# needed):** PR1 → PR6 → PR10 → PR12 → PR14 → PR15.

Mutation testing (PR5.5, #24) gates the emitter TCB once its goldens exist; it is
an on-demand / periodic run (`just mutants`), not a per-PR gate, because mutation
runs are slow.

## Open decisions

Emission-shape / canonicalization decisions (**D1–D5**) are **resolved** by
adopting CTSC 0.1; see [`docs/trace-contract.md`](trace-contract.md).

1. Macro naming: `watch_*` (final — **D1**).
2. Artifact format: **CTSC OTLP JSON** (`.otlp.json`/`.otlp.jsonl`) — non-finite
   floats project to a tagged union, so JSON is faithful; owned by PR11. ✅
3. Ship contract-diff before trace-diff (recommended — metadata drift is the
   cheap, always-available, coverage-independent first product).

## Related SpecGate issues (context)

- #36 — trace-engine trust anchor + trace contract (the extraction TCB; source
  of the goldens/encoder tests/canonicalization findings).
- #26 — differentiation from TDD / spec-as-data (refactor-oracle = the compare
  idea).
- #30 — proptest inputs → trace diff (the trace-coverage feeder; north-star).
