# Golden Trace Corpus — Fixture Catalog

Driftwatch's trace pipeline is documented **by example**. Each golden here is a
byte-exact CTSC 0.1 trace produced by running **real annotated fixture code**
through the macros + runtime and serializing it with the emitter. When this
catalog (or any prose doc) disagrees with a fixture, **the fixture is the source
of truth**.

This corpus is **language-neutral**: the golden `.json`/`.jsonl` files live at
the repository root so the Rust and future C# emitters bless against the same
bytes. Each language backs a golden with its own annotated fixture code that
must serialize to the identical trace (cross-language conformance). The Rust
fixtures are a reusable library in the `golden` crate
([`rust/crates/golden/`](../../rust/crates/golden/)); its integration test is
the harness that drives them.

## How a golden is produced

The harness, per fixture:

1. `reset()`, then opens a `conformance.run` + `conformance.scenario` frame via
   `open_span` — this framing is the extraction driver's eventual job; until the
   `extract` crate exists, the harness supplies it.
2. Invokes the **real annotated fixture operations** (`#[watch_operation]`,
   `watch_dep!`, `watch_point!`, `#[derive(Watchable)]`, panic→fault) inside the
   scenario. Their spans, inputs, events, values, and faults come from actual
   macro expansion — nothing is hand-shaped.
3. Drains `take_spans()`, pins each span's `trace_id` to a fixed constant (the
   trace id is an opaque correlator the comparator ignores; the per-thread trace
   counter is otherwise nondeterministic across a parallel test run), pairs the
   spans with a fixed `Resource`, and serializes via the emitter.
4. Byte-compares against the on-disk golden, or — under `DW_BLESS=1` —
   regenerates it.

```
# compare
cargo test -p golden --features trace
# regenerate (review the diff before committing)
DW_BLESS=1 cargo test -p golden --features trace
```

Span-id and tick counters re-zero on `reset()`, so everything except the trace
id is deterministic by construction; the harness pins the trace id.

## Role in the test suite

Goldens are the **whole-pipeline wire oracle** — the non-circular kill oracle
for mutation testing (#24). A byte-golden fails on *any* output change, so a
surviving mutant means an unexercised clause. Each fixture is **one focused
scenario asserting one behavior**, named `<subject>_<condition>_<result>` in
[`tests/golden.rs`](../../rust/crates/golden/tests/golden.rs); a fixture that
spans several operations does so only when the *relationship* between them is
the behavior under test (span linking/ordering, a fault cascade, JSONL framing).

Unit tests in [`rust/crates/artifact/tests/emit.rs`](../../rust/crates/artifact/tests/emit.rs)
remain the emitter-level microscope for invariants a byte-golden cannot express
(see [Covered by unit tests](#covered-by-unit-tests)).

## Fixtures

Each fixture is produced by the like-named test in
[`tests/golden.rs`](../../rust/crates/golden/tests/golden.rs).

| Fixture | Test / behavior | `validate.py` |
|---------|-----------------|:---:|
| `operation-result.otlp.json` | `operation_returning_value_records_result` | ✅ |
| `operation-empty.otlp.json` | `operation_returning_none_records_empty` (`Option::None`, OK) | ✅ |
| `operation-unit.otlp.json` | `operation_returning_unit_records_no_completion` (`()`, OK) | ✅ |
| `operation-error.otlp.json` | `operation_returning_err_records_error_status` (`#[derive(Watchable)]` error) | ✅ |
| `dep-ok.otlp.json` | `dep_returning_ok_records_result` (`watch_dep!` Ok) | ✅ |
| `dep-err.otlp.json` | `dep_returning_err_records_error` (`watch_dep!` Err, Error status) | ✅ |
| `sequential.otlp.json` | `chained_operations_link_and_order_spans` (parent/child linking, ordering, observation) | ✅ |
| `values.otlp.json` | `watch_point_encodes_ctsc_value_edges` (every §8 value type and edge) | ✅ |
| `target-fault.otlp.json` | `operation_panic_records_target_fault` | ✅ |
| `fault-cascade.otlp.json` | `dep_panic_cascades_fault_to_all_frames` | ✅ |
| `supervisor-fault.otlp.json` | `supervisor_fault_records_on_scenario` — **harness-emitted** (no annotation produces a supervisor fault; deferred to a real supervisor, #14+) | ✅ |
| `streaming.otlp.jsonl` | `jsonl_serializes_one_capture_per_line` (multi-capture JSONL framing) | ⚠️ per-line |

The `.otlp.json` captures are gated by the upstream CTSC oracle
`docs/ctsc/validate.py`; `.otlp.jsonl` is validated line by line.

## What each clause is tested by

Rows are the profile clauses from [`docs/trace-contract.md`](../../docs/trace-contract.md);
the Golden column is where each is pinned end-to-end.

### Annotation → span

| Clause | Golden |
|--------|--------|
| run span (harness-supplied framing) | every fixture |
| scenario span with `name` + `index` | every fixture |
| `watch_operation` → operation span with `component.id` / `operation.name` / `operation.inputs` | `sequential` |
| `watch_dep` → nested operation | `dep-ok` |
| root omits `parentSpanId`; children link to parent | `sequential`, `dep-ok` |
| deterministic span ordering | `sequential` |
| component inherited by a dep whose component is `None` | `dep-ok` |

### Events (completions)

| Clause | Golden |
|--------|--------|
| `conformance.observation` (`watch_point!`) | `sequential`, `values` |
| `conformance.result` (success value) | `sequential`, `operation-result` |
| `conformance.empty` (`Option::None`, OK status) | `operation-empty` |
| unit / `()` → no completion event, OK | `operation-unit` |
| `conformance.error` (`#[derive(Watchable)]` variant name + value, Error status) | `operation-error` |
| `conformance.error` on a dep child span (`Err` disposition, fallback name, Error status) | `dep-err` |

### Status

| Clause | Golden |
|--------|--------|
| `Unset` → OK promotion (emitter-owned) | `sequential` |
| Error status on a declared error | `operation-error`, `dep-err` |
| Error status on a fault, no completion event | `target-fault` |

### Values (CTSC §8, via real `ToValue`)

| Clause | Golden |
|--------|--------|
| string / bool scalars | `values` |
| integer, incl. edges `0` / `±` / `i64::MIN` / `i64::MAX` | `values` |
| float finite: fractional / `2.0` / `-0.0` | `values` |
| float non-finite: `±Inf` and NaN (two bit-patterns → canonical `"NaN"`) | `values` |
| list: empty + nested | `values` |
| set: empty + sorted-order emission (`BTreeSet`) | `values` |
| map: empty + nested (`BTreeMap`) | `values` |
| variant: payload + unit | `values` |

### Faults (producer choices)

| Clause | Golden |
|--------|--------|
| target fault: panic → `type="unexpected"`, `observer="target"` | `target-fault` |
| fault cascade: recorded on the operation **and** its enclosing dep | `fault-cascade` |
| supervisor fault: `observer="supervisor"`, on a scenario span (harness-emitted) | `supervisor-fault` |

### Serialization & constants

| Clause | Golden |
|--------|--------|
| `.otlp.json` framing | every `.json` fixture |
| `.otlp.jsonl` multi-capture framing, newline-terminated | `streaming` |
| `scope.name` / `scope.version` / `schemaUrl` | every fixture |
| required resource attrs (`conformance.version` = `0.1.0`, `tool.*`, `target.*`) | every fixture |
| ids lowercase-hex and nonzero | every fixture |
| `dropped*Count` omitted; `events` omitted when empty; `status` is `{code}` only | every fixture |

## Covered by unit tests

Invariants the byte-goldens cannot express stay in `emit.rs`:

| Invariant | Why not a golden |
|-----------|------------------|
| `SpanEvent` equality ignores `time` | equality semantics, not wire bytes |
| `end`-fallback for an unclosed span | unreachable through a normal capture |
| `DW_BLESS` round-trip regeneration | harness behavior, not a fixture |

Emitter branches that the goldens now exercise end-to-end (AnyValue mapping,
status codes, id/scope/schema shape) may have their redundant `emit.rs` asserts
thinned — conservatively, and only where a golden truly covers them.

## Deferred

| Fixture / feature | Blocked on |
|-------------------|------------|
| `parallel`, `cross-process` | #42 (parallel / cross-thread capture) |
| annotation-sourced supervisor faults + detail attrs (`fault.phase`, `fault.exit_code`, `fault.operation.*`, `status.message`) | #14+ (a real supervisor producer; the runtime models faults as `type`/`observer`/`message` and status as `{code}` only today) |
| end-to-end framing via the real extraction driver (currently harness-supplied) | the `extract` crate (a later roadmap PR) |

## Provenance

Fixture shape and required semantic attributes mirror SpecGate's CTSC corpus
(`docs/ctsc/corpus/trace/valid/`), documented there in
`docs/knowledge/fixtures.md`. Composite valid captures are gated by the upstream
CTSC oracle `docs/ctsc/validate.py`.
