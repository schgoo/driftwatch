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

**One artifact per code-version extraction**, two sections:

- `contract` — the normalized schema (registry → spec types).
- `traces` — **keyed runs**: `{ key, inputs?, events[] }`. The **key** (op +
  canonicalized inputs) is load-bearing: it aligns run-for-run across versions.

Two producing mechanisms (already exist in SpecGate as `discover` /
`run_spec`): static registry for the contract, dynamic buffer for traces. Two
diff algorithms: structural set-diff (contract) and keyed sequence/value diff
(traces). The contract-diff **contextualizes** the trace-diff.

## Salvage map (from SpecGate)

| Take (▪ lift · ⚠ refactor) | Leave behind |
|---|---|
| ▪ `specgate-runtime` → `runtime` (buffer, `Value`, `SpecEvent`, registry) | matcher + operator catalog (`$gt`, `$unordered`, …); the lossy `serde_json` record mode |
| ▪ `specgate-annotations(-macros)` | `expected:` cases, narrative/level/provenance |
| ⚠ `specgate-types` → `contract` (strip `cases`) | self-host / conformance authored ledgers |
| ⚠ harness driver slice: binding-resolve + codegen + build+run+collect + `discover` | `validate` case-runnability, `extract --cases` |
| ▪ C# `SpecGate.Weaver` + `SpecGate.Runtime` | spec-case codegen, the matcher tail of `run_spec` |
| ▪ the #36 trace goldens + encoder edge tests (the extraction TCB) | |

Trust anchor carried over: the #36 work (exact-trace goldens via derived
`Debug`, `Value` encoding edge tables, canonicalization findings F1 Set≠List /
F2 NaN·Inf / F3 loose `PartialEq`). Canonicalization is now **load-bearing**:
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
- **PR2** ✅ — `runtime` pt1: `Value` (strict structural `Eq`/`Ord`, `Debug`) + `TraceEvent` + `ToValue`; #36 encoder edge tests. Dropped SpecGate's loose equality and hand-written serde (serialization deferred to the artifact-format PR). ▪
- **PR3** — `runtime` pt2: buffer, `emit_*`/`take_traces`/`reset`, mock table, registry (`OpMeta`/`TypeMeta`/discovery), `SpecEvent`. **No serialization** (no JSONL record mode — deferred to the artifact-format PR). ▪ ~400
- **PR4** — `annotations(-macros)` + facade re-exports. *(decision: keep `spec_*` names or rename `dw_*`)* ▪ ~450
- **PR5** — minimal fixtures + lift the 51 trace goldens (emission trust anchor). ▪ ~400
- **PR5.5** *(gh #24)* — **mutation testing** capstone: `cargo-mutants` scoped to
  the emitter TCB (`runtime` + `annotations(-macros)`), run against the native
  encoder tests + trace goldens only (non-circular oracle), survivors triaged to
  zero. The "measure the trust anchor" gate — SpecGate #36 Rung 4 analog. Depends
  on PR5 (the goldens are the kill oracle). ✚

### Phase 1.5 — artifact format (reprioritized; load-bearing)
- **PR11** *(moved earlier from Phase 4)* — **artifact serialization format** +
  keyed trace model. The format must round-trip `Value`/`TraceEvent` faithfully
  (Set≠List, `NaN`/`Inf`/`-0.0`), so JSON is unsuitable: use **derived** serde
  over a **binary self-describing** format (CBOR/postcard/bincode), never
  hand-written serde. Keyed runs `{key, inputs?, events[]}` + key derivation.
  Foundational — every persistence path (record, collect, snapshot, compare)
  depends on it. ✚ load-bearing

### Phase 2 — contract
- **PR6** — `contract`: lift types, strip `cases`, keep name/types/operations/binding + validation; `.spec.yaml`→`.contract.yaml`. ⚠ ~450

### Phase 3 — extraction driver (drop the matcher)
- **PR7** — binding resolution (drop matcher bits). ▪ ~350
- **PR8** — runner codegen: drive ops → emit traces (artifact format, no `expected:`/matcher). ⚠ ~450
- **PR9** — build+run+collect: invoke cargo/dotnet, capture keyed traces (`run_spec` front half minus match tail). ⚠ ~400
- **PR10** — contract extraction: `discover` slice (registry → normalized schema). ▪ ~350


### Phase 5 — diff (new)
- **PR12** — contract-diff: structural diff + breaking-change classification + report. ✚ ~450
- **PR13** — trace-diff: align keyed runs, canonical event divergence, first-divergence report. ✚ ~450

### Phase 6 — CLI (new)
- **PR14** — CLI skeleton + `snapshot` (extract → artifact). ✚ ~300
- **PR15** — `compare A B` + `--format human/json/llm`. ✚ ~350
- **PR16** — `--mode full/diff/pr`, `--base`, `--pr` orchestration. ✚ ~400

### Phase 7 — C# backend (parallelizable after Phase 1)
- **PR17** — lift `SpecGate.Runtime` (C# emitter + trace JSON). ▪ ~400
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

1. Macro naming: keep `spec_*` or rename `dw_*`.
2. Artifact format: binary self-describing (CBOR/postcard/bincode) via derived serde — must be faithful (Set≠List, floats); extension (`.dw`?). Owned by the artifact-format PR (#11). ✅ direction set (see #11).
3. Ship contract-diff before trace-diff (recommended — metadata drift is the
   cheap, always-available, coverage-independent first product).

## Resolved decisions

- **Synthetic return event name (#31): keep `$result`.** The `$` reserves a
  compiler-synthesized namespace and prevents confusion with user-named
  parameters or fields. This decision preserves the existing event vocabulary;
  it does not change trace structure, ordering, encoding, or canonicalization.

## Related SpecGate issues (context)

- #36 — trace-engine trust anchor + trace contract (the extraction TCB; source
  of the goldens/encoder tests/canonicalization findings).
- #26 — differentiation from TDD / spec-as-data (refactor-oracle = the compare
  idea).
- #30 — proptest inputs → trace diff (the trace-coverage feeder; north-star).
