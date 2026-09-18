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

Capture is execution-free: each side of a compare is an artifact dir written by
the project's *own* `cargo test --features driftwatch` run (registry + trace,
see Phase 3). `compare <A> <B>` diffs two such dirs. Automated multi-ref modes
(`--mode diff --base main`, `snapshot` of another ref) obtain each version's
artifacts from a suite run on that ref (developer or CI) rather than by having
Driftwatch build the ref itself. Driftwatch building another version is
out of scope.

## Artifact format

Driftwatch emits **CTSC 0.2** (see `docs/trace-contract.md`). One extraction
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

Driftwatch emits **CTSC 0.2** (Conformance Trace Semantic Conventions):
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
| ⚠ harness driver slice: global span sink + exit-flush (auto-emit) + `discover` | binding *commands* / runner codegen / build+run+collect; `validate` case-runnability, `extract --cases` |
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
- **#1** ✅ — driftwatch scaffold: `rust/` workspace, `rust/crates/`, justfile
  (build/test/clippy/fmt + **`evaluate`**), Rust-only CI, licenses, `.gitignore`,
  toolchain, README — **plus the agent loop** (`AGENTS.md` +
  `.github/agents/*.agent.md` observe-plan-act-verify orchestrator +
  planner/implementer/reviewer specialists + `.github/skills/`) and the
  **planning-doc tree** (`docs/README.md`, `docs/agentic-loop.md`,
  `docs/digests/{human,llm}.md`, `docs/references.md`) — modeled on
  `~/repos/dcf`. Wire **`cargo evaluate`** into the `check` recipe + CI as a
  harness gate (with a starter `evaluate.toml`). ✚ ~450 **Delivered in #21.**

### Phase 1 — emitter (extraction TCB)

All emitter work implements a clause of [`docs/trace-contract.md`](trace-contract.md).

- **#2** ✅ — `runtime` pt1: `Value` (strict structural `Eq`/`Ord`, `Debug`) + `TraceEvent` + `ToValue`; #36 encoder edge tests. Dropped SpecGate's loose equality and hand-written serde (serialization deferred to the artifact-format item). ▪
- **#3** — `runtime` pt2: buffer, `emit_*`/`take_traces`/`reset`, registry (`OpMeta`/`TypeMeta`/discovery), `SpecEvent`. **No serialization** (deferred to #11). ▪ ~400
- **#4** ✅ — core annotations: `#[watch_operation]` + `#[watch_input]` + `watch_point!` + facade re-exports; per-parameter inputs, field-mutation echo, `$result` via `ReturnEmit` (retired in #37). Macro surface committed as `watch_*` (D1). ▪
- **#29** ✅ — `#[derive(Watchable)]`: structural struct/enum emission (includes the Watchable merge). Unblocks the struct/enum outcome clauses. ▪
- **#30** ✅ — dependency-boundary tracer: per-arg inputs + real-call `.response`/`.error`, optional `?`. Observation-only (substitution dropped → #27). The original `#[watch_dep]` *attribute* form is **retired** — superseded by the expression-position `watch_dep!("name", <expr>)` function-like macro (transparent observer; unifies with `watch_point!`; non-call shapes are value-only, no `compile_error!`). ▪

- **Emission contract ratified** ✅ — `docs/trace-contract.md` adopts CTSC 0.2 as Driftwatch's producer profile; D1–D5 resolved. Prereq for the emission items and the golden corpus.
- **#37** ✅ — CTSC completion events: operation completion emits as
  `conformance.result`/`.empty`/`.error`, and dependencies as **nested
  `conformance.operation` spans** (own inputs + completion). Value carries via
  one unified `ValueEmit` ladder (`runtime/src/value_emit.rs`) returning a
  `Value`, shared by the result and error dispositions; `split_error`
  maps a decomposed error to `error.name`/`error.value`. Folds in **#44**
  (author-declared `component`) and **#45** (structural error decomposition).
  Retires the flat `WatchEvent` path and the `Map` `{Ok|Err}`/`{Some|None}`
  return tags. ▪
- **#38** ✅ — dependency initializer shapes, folded into the `watch_dep!` macro
  redesign: `?` and combinators now compose **outside** the macro (author-placed),
  `.await` is peeled/re-applied **inside**, and async deps use the panic-fault
  `catch_unwind_fut` cascade. Also **subsumes `e1-unify`** ("all value types on
  deps"): the `DepObserve` autoref ladder (`runtime/src/dep_observe.rs`) dispatches
  `Result`/`Option`/plain-`T` dispositions on the observed runtime value, so every
  value shape is handled with no per-shape `compile_error!`. ▪
- **#39** ✅ — panic disposition: on a caught panic emit `conformance.fault` (+ partial trace, `ERROR` status), no result. ✚
- **#40** ✅ — input surface: capture `&mut` non-receiver params as inputs (pre-call value at span open) — supersedes #4's `&mut` exclusion; receivers stay excluded and hidden inputs stay annotator-captured via `watch_point!`. Updated `operation_params` goldens. Whole-value deref-mutation observation (e.g. `*total += …`) remains a follow-up. ▪
- **#42** — parallel branches: emit `conformance.parallel` with **unordered** child branches from concurrently-executing operations/dependencies. Defines cross-thread `SpanContext` propagation with **globally-unique `span_id`s** (lane-encoded: per-thread lane ∥ per-capture counter) so merged multi-thread spans keep unambiguous `parent_span_id` linkage; single-threaded captures stay byte-deterministic. Deferred from #37a, whose span substrate is single-threaded. Comparator pairs branches by identity, not order. ✚ ~350 · **load-bearing** (id-minting scheme)
- **#53** — async/concurrent `SpanContext` propagation: the span substrate is
  thread-local (`runtime/src/span.rs` `SPANS`/`STACK`), so the active-span
  context is lost across an `.await` thread hop and across `spawn`/thread/channel
  handoff — a `watch_dep!` in a future resumed on another worker mis-parents.
  Adds a capture-and-re-enter `SpanContext`, a concurrency-safe **per-run
  collector** (replacing the thread-local sink), unique spans per retry/repeated
  invocation, and cancellation/incomplete-operation disposition. Distinct from
  #42 (span-id *minting*); this is context *propagation* and depends on it.
  Origin: SpecGate feature-requests §3. ✚ ~400 · **load-bearing** (span identity)
- **#54** — `Value` typed fidelity: add `Value::Bytes` (binary payloads
  currently degrade to a `List` of ints) + a native null/unit scalar (unit is
  today an empty `Map`); apply export-time depth/count/size **bounds** and a
  **redaction** hook before OTLP serialization (clears the evaluate
  `M-LOG-STRUCTURED` secrets/PII warning); add a non-interference test
  (forward-exactly-once). Int-width→`i64`/float→`f64` canonicalization (F3)
  stays. Ties to #40's whole-value deref-mutation follow-up. Mirrors the
  upstream SpecGate value gap (`schgoo/specgate#37`). Origin: SpecGate
  feature-requests §4. ✚ ~350 · **load-bearing** (adds an `Ord`-space variant)

- **#5** ✅ — the trace **golden corpus** as CTSC `.otlp.json` fixtures (per D5) covering every profile clause; re-author the SpecGate `mock_*` goldens as real-dependency (nested-operation) observation. Reuse SpecGate's CTSC corpus + `validate.py`. The emission trust anchor + the ongoing TDD spec (new clause → golden → implement). ▪ ~400
- **#24** ✅ — **mutation testing** capstone: a HYBRID gate over the emitter
  TCB, run against the runtime encoder tests + trace goldens only (non-circular
  oracle — checked-in bytes, never regenerated from a mutant). **cargo-gamma**
  mutates the `runtime` emitter (`rust/gamma.toml`: `packages = runtime`,
  `test-packages = runtime + golden`, `all-features` so the `trace` goldens run,
  `min-score = 95`); **cargo-mutants** mutates the `annotations(-macros)` surface
  (`rust/.cargo/mutants.toml`) because gamma instruments once and toggles mutants
  at test runtime, so it cannot mutate a proc-macro that runs at the golden
  crate's compile time. On-demand via `just mutants` (sets `DW_GOLDEN_DIR` for
  gamma's sandbox), not a per-PR gate (slow). Runtime run: **206 mutants → 206
  killed, 0 survived, 0 uncovered = 100.0%**; a handful of mathematically-
  equivalent / comparator-ignored sites carry documented `// #[gamma::skip(…)]`
  directives. `registry.rs` (static-contract JSON) is excluded — its oracle is
  the contract/discover work (**#6/#10**), tracked as a follow-up. The "measure
  the trust anchor" gate — SpecGate #36 Rung 4 analog.
  Depends on #5 (the goldens are the kill oracle). ✚

### Phase 1.5 — artifact format (reprioritized; load-bearing)
- **#11** ✅ — **CTSC OTLP emitter**: serialize the runtime buffer to CTSC Trace
  (`.otlp.json`/`.otlp.jsonl`) and `Value`→OTLP `AnyValue` per profile §Values
  (`u64`→decimal string, set→stable-ordered array, non-finite float→OTLP
  `doubleValue` string with NaN payloads normalized). Foundational — every
  persistence path (record, collect, snapshot, compare) depends on it. ✚
  load-bearing

### Phase 2 — contract
- **#6** ✅ — `contract`: model, parse, and validate the **CTSC Registry 0.2**
  document (`ctsc.registry`, JSON) — the static contract `discover` (#10)
  generates. Serde model over `registry.md` + `ctsc-registry-0.2.schema.json`;
  cross-item validation (§4.1/§7/§8/§10) the JSON schema can't express. The
  artifact is `*.registry.json`, not the pre-CTSC `.contract.yaml`. Import-file
  loading + sha256 digest verification + cross-document type resolution (§6) are
  deferred to a linked-validation follow-up. ⚠ ~450 **Delivered in #57** (serde model + `parse` + cross-item `validate`).
- **#6.1** *(deferred; lower priority than the extraction chain)* — **linked
  (multi-document) registries**: import loading, `sha256:` digest verification,
  and cross-document named-type resolution (CTSC §6, §8 imported branch). Single
  `discover` output is one self-contained document, so nothing on #6–#15 emits
  imports; this only matters once a capture references a *separately-captured*
  dependency registry. **Open question first:** what comparison features does
  registry linking actually buy (e.g. detecting drift in a pinned dependency's
  contract via digest, cross-version type-identity)? Scope it against Phase 5
  diff value before building. ✚

### Phase 3 — auto-emit on test run (Driftwatch executes nothing)

Capture happens as a **side effect of the project's own test run**:
`cargo test --features driftwatch` links the runtime, the tests exercise the
annotated ops, and on process exit the runtime flushes a **registry** + **trace**
to an output dir. Driftwatch runs none of the project's commands: the only code
that runs is the project's own suite. There is no command-execution surface.

- **#7** ✅ — **capture config + output contract**: a repo-root `driftwatch.toml`
  read by both the emitter (where/how to write) and the CLI (where to read).
  Keys:
  - `outdir` — artifact dir, default `target/driftwatch/`; `DRIFTWATCH_OUT_DIR`
    overrides. Precedence: env var > `outdir` > default.
  - `[target] name` — the `conformance.target.name` **label** for the run
    (optional; default derived from `CARGO_PKG_NAME`, caller-overridable). CTSC
    0.2 defines it as a run label, **not** a pairing key — a compare pairs on
    `conformance.component.id` (§Comparison), so target.name carries no
    correlation weight and need not be language-neutral. It is still emitted
    unconditionally (CTSC requires the resource attribute present);
    `target.language` stays emitter-fixed (`"rust"` here).
  - `format` — trace serialization, `json` | `jsonl` (maps to `OtlpFormat`).
  - `clean` — wipe `outdir` before a run so a prior run's artifacts can't mix
    into a capture; default off.

  Artifact file names are fixed convention (`registry.json`,
  `trace.otlp.jsonl`), not configurable. Parser is feature-gated so it stays out
  of production builds. ▪ ~250 **Delivered in #59** (`artifact::CaptureConfig`).
- **#8** ✅ — **export on span close** (OTel `SimpleSpanProcessor` model): when a
  thread's root span closes (the `STACK`-empties branch of `SpanGuard::drop`),
  drain that thread's completed span tree and **append** it as one
  `TracesData` line to `trace.otlp.jsonl` in the resolved `outdir`. The file
  stays current, so there is no end-of-process flush and no process-exit hook —
  hence no `unsafe` (Rust has no safe stable atexit; `ctor`/`libc::atexit` are
  FFI/linker tricks we avoid). A global `OnceLock<Mutex<Writer>>` serializes
  appends so lines from concurrent test threads never interleave. `runtime`
  cannot call `artifact` (the dep points the other way), so `runtime` exposes a
  **sink callback** (`OnceLock<fn(&[Span])>`) the feature-gated emitter glue
  registers; `SpanGuard::drop` invokes it on root close. Emitter always writes
  `jsonl` (the CTSC §2 File Exporter shape); `format = "json"` is a CLI/snapshot
  concern, not the live path. Feature-gated on `driftwatch`. ⚠ ~450 **Delivered in #60.**
- **#10** ✅ — **registry emission**: derive a CTSC `RegistryDocument` from the
  link-time discovery metadata and write `registry.json` in the `outdir` once
  per run (a `OnceLock` on first span export), validated against the #6
  `contract` crate and goldened. Split into **10a** (a pure `extract::derive`
  driver over the typed `runtime::OpMeta`/`TypeMeta` slices — not
  `runtime::discovery_json`, so `runtime` stays untouched) and **10b** (live
  emit + `[registry] version` / `DRIFTWATCH_VERSION` identity config + golden).
  `derive` sorts components by id and operations/types by name so the artifact
  is byte-reproducible across platforms (link-time order is not stable). ▪ ~250 **Delivered in #63 (10a) + #65 (10b).**
  - **Phase R (resolver-fed front-half):** #10 is the issue Phase R supersedes.
    Its build-time **string** type-resolution front-half — `10a`'s
    `extract::derive` string driver, i.e. `extract::classify_return` /
    `parse_type_ref` — is **replaced** by the RA static resolver, which produces
    already-resolved `contract` types (`TypeRef` / observations / dependencies)
    that feed `derive` directly (R-b redefines the `derive` seam per Gate #3;
    R-c/R-d supply the resolved input). #10's *output* contract and its
    byte-reproducibility guarantees are **unchanged** — only the type-resolution
    front-half is swapped. Downstream Phase 5/6 (#12–#16) consume whatever
    contract `derive` emits and are untouched by Phase R. See
    [`docs/decisions/phase-r-static-resolver.md`](decisions/phase-r-static-resolver.md).


### Phase R — Static Resolver (rust-analyzer type inference)

Replaces the build-time **string** type pipeline (`extract::classify_return` /
`parse_type_ref`) with an RA-backed **static resolver** that does real type
inference, so type-aliased error channels are recovered. Inserted **after Phase
3** per Gate #1. Annotations stay *selection / aiming* only. Full rationale,
ratified gates, and the deferred-item record:
[`docs/decisions/phase-r-static-resolver.md`](decisions/phase-r-static-resolver.md).
Feasibility proof (**GO, with constraints**):
[`docs/spikes/ra-extract/REPORT.md`](spikes/ra-extract/REPORT.md). Acceptance
oracle: `tests/golden/resolved-registry.json` +
`rust/crates/golden/tests/resolver_pending.rs`
(`resolver_reproduces_resolved_registry`, red-by-design `#[ignore]`).

**Ratified gates (do not reopen — see the decision record):**

- **Gate #1 (placement)** — new Phase R after Phase 3; **supersedes the
  build-time string type-resolution front-half of #10** (registry emission —
  `10a`'s `extract::derive` string driver: `classify_return` / `parse_type_ref`).
  #10's output contract and byte-reproducibility are unchanged; only the
  front-half is replaced (see the note on the #10 entry). Downstream Phase 5/6
  consumers (#12–#16) are untouched.
- **Gate #2 (dep/toolchain)** — RA is feature-gated under the same feature that
  gates tracing/extraction (`trace` / `driftwatch`); with tracing off,
  `ra_ap_*` and its rustc-1.96 pin are never pulled. When enabled, RA runs on
  **every** extraction — the **default** path, not an add-on.
- **Gate #3 (derive seam)** — `extract::derive()` is redefined to accept
  already-resolved `contract` types (`TypeRef` / observations / dependencies)
  and relocated to the registry/contract layer, shedding the string parsers
  (R-b).
- **Gate #4 (ty)** — errors stay **name-only** (`ErrorOutcome.ty` = `None`);
  cross-language foreign-error name canonicalization is **DEFERRED**
  (trace-contract-owned).

**PR sequence (≤500 net LoC each):**

| PR | Scope |
|---|---|
| **R-a** *(doc-only)* | Track the spike into `docs/spikes/ra-extract/`; write the decision record; add this roadmap stub + renumber/cross-reference notes. No crate/test/oracle changes. |
| **R-b** | Redefine `derive()` over resolved `contract` types (Gate #3), relocate to the registry layer, adapt the legacy string path. Harness stays green. |
| **R-c** | New feature-gated resolver crate: pinned `ra_ap_*=0.0.349` + `unicode-ident=1.0.22`, `ProcMacroServerChoice::None` + source pre-scan; resolve a target to resolved observations. Adapts the spike PoC. |
| **R-d** | Wire the resolver into `extract` as the **default** path (runs every extraction, feature-gated, Gate #2); `watch_point!` / `watch_dep!` token→expr mapping (PARTIAL risks flagged in the spike). |
| **R-e** | Delete the red-by-design `#[ignore]` on `resolver_reproduces_resolved_registry`; retire the superseded string parsers. **The oracle flipping green is the acceptance gate.** |

### Phase 5 — diff (new)
- **#12** — contract-diff: structural diff + breaking-change classification + report. ✚ ~450
- **#13** — trace-diff: CTSC comparison under the **Strict** policy (`ctsc.strict/0.1.0`, optional). Comparison indexes on `conformance.component.id`, not `target.name`: within a component, operations pair **by position**, and paired operations must share `component.id` / `operation.name` / inputs; repeated invocations of one operation pair by order. Diff events/values; first-divergence report. A reorder-resilient identity-keyed variant is deferred to the comparison-engine build. ✚ ~450

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

Linear within phases: 1→2→3→R→5. Phase 2 independent after scaffold. Phase 7
branches after Phase 1. **Phase R** sits after Phase 3 and re-backs **#10's**
type-resolution front-half with the RA resolver; #10 still emits the same
contract (byte-reproducible), so downstream Phase 5/6 (#12–#16) are untouched
consumers. Phase R's own PRs are linear R-a→R-e (R-b→R-c→R-d build the
seam/crate/wiring, R-e flips the acceptance oracle green).

**Fastest path to a real demo (contract-diff — cheap, un-gameable, no trace
driver or C# needed):** #1 → #6 → #10 → #12 → #14 → #15.

Mutation testing (#24) gates the emitter TCB once its goldens exist; it is an
on-demand / periodic run (`just mutants`), not a per-PR gate, because mutation
runs are slow.

## Ratified decisions

Emission-shape / canonicalization decisions (**D1–D5**) are **resolved** by
adopting CTSC 0.2; see [`docs/trace-contract.md`](trace-contract.md).

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
- **Phase R (static resolver)** — replace the build-time string type-resolution
  front-half of **#10** (`extract::derive`'s `classify_return` / `parse_type_ref`)
  with an RA-backed resolver; #10's output contract is unchanged. Four gates
  ratified (placement / dep-gating / `derive()` seam / name-only `ty`) with
  foreign-error canonicalization deferred. Full record:
  [`docs/decisions/phase-r-static-resolver.md`](decisions/phase-r-static-resolver.md).

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