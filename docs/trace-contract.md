# Driftwatch — CTSC producer profile

Driftwatch emits **CTSC 0.1**. This document specifies only what Driftwatch
decides within CTSC's producer latitude — the annotation→CTSC mapping and the
source-language choices CTSC leaves open. CTSC's `trace.md`, `registry.md`, and
`comparison.md` are **normative**; anything they fix is not restated here.

Driftwatch has two conformant producers — the **Rust** runtime and the **C#**
weaver — that emit the same CTSC. This profile is language-neutral; per-language
constructs appear as examples. Annotation names are written bare (`watch_operation`);
each producer spells them idiomatically (`#[watch_operation]`, `[WatchOperation]`).

**Status:** adopting CTSC 0.1 (draft). Sequencing lives in `docs/roadmap.md`.

## Artifacts

| Driftwatch | CTSC |
|---|---|
| behavioral traces | Trace — OTLP `TracesData` (`.otlp.json` / `.otlp.jsonl`) |
| contract (link-time registry) | Registry document (`ctsc.registry`) |
| contract-diff + trace-diff | Comparison policy |
| golden corpus (#5) | Trace corpus (`.otlp.json`) |

## Annotation → span

| Driftwatch construct | CTSC span | Domain attributes |
|---|---|---|
| test-runner invocation (`cargo test`, `dotnet test`) | `conformance.run` | — |
| each observed test | `conformance.scenario` | name, index |
| `watch_operation` function/method | `conformance.operation` | `component.id`, `operation.name`, `operation.inputs` |
| `watch_dep` call | nested `conformance.operation` | a dependency *is* a nested operation |
| constructor (`watch_operation`) | nested `conformance.operation` | the built object is its `result` |

Resource attributes: `conformance.tool.{name,version}`,
`conformance.target.{name,language}`, and (Linked) `conformance.registry.*`.

## Inputs

Driftwatch observes the **full input surface** — any value that determines
behavior. Each maps to one `operation.inputs` entry; `watch_input("name")`
overrides the key.

| Parameter kind | Captured |
|---|---|
| by-value / shared read (Rust `T`/`&T`, C# value / `in`) | the value |
| exclusive/mutable, non-receiver (Rust `&mut T`, C# `ref`/`out`) | its **pre-call** value; later mutations are observations |
| receiver (Rust `self`/`&self`/`&mut self`, C# `this`) | **excluded** — captured via the constructor's operation |
| ambient (globals, statics, config, env) | not signature-visible → capture at the read site as an observation |

Dependency arguments are the nested operation's own inputs (named by identifier,
else positional); its receiver is excluded.

## Events

| Driftwatch construct | CTSC event |
|---|---|
| body echo / `watch_point` / field-mutation echo | `conformance.observation` |
| success value (Rust `Ok`/`Some`/plain, C# non-null return) | `conformance.result` |
| deliberate absence (Rust `None`, C# `null`/empty `Nullable`) | `conformance.empty` |
| unit / no value (Rust `()`, C# `void`) | *(none; `OK`)* |
| declared error (Rust `Err`, C# exception declared in the registry) | `conformance.error` (registry-declared `name`) |
| unexpected failure (Rust `panic!`, C# exception *not* in the registry) | `conformance.fault` (`observer=target`) |

Nested outcome types peel to their disposition — e.g. Rust `Result<Option<T>, E>`:
`Ok(Some)`=result, `Ok(None)`=empty, `Err`=error.

## Values

Values are encoded per CTSC trace §8. Source-type resolution:

| Source type | CTSC canonical value |
|---|---|
| sum / tagged type (Rust enum / `Option` / `Result`, C# enum / nullable / union) | tagged union (variant → payload) |
| record / struct / class | string-keyed map |
| tuple | list |
| set / unordered collection (Rust `HashSet`/`BTreeSet`, C# `HashSet`) | set — emitted in stable (sorted) order |
| ordered collection projected as list | list — deterministic order preserved |
| float (`f32`/`f64`, C# `float`/`double`) | double — finite as a number; non-finite as OTLP `"NaN"`/`"Infinity"`/`"-Infinity"` (CTSC §8.4) |

Tagged-union variant labels carry no type identity; that lives in the registry
(Linked), so two types sharing a variant label are equal at Trace Core.

## Producer choices

1. **Dependencies are nested operations** — `watch_dep` emits a child
   `conformance.operation` (own inputs + completion); there is no separate
   dependency event vocabulary.
2. **Inputs are a span attribute**, not per-input events.
3. **Component is author-declared.** `conformance.component.id` is declared on
   the annotation surface (`component = "…"`, **mandatory** on
   `watch_operation`); it is language-agnostic (a Rust and a C# implementation of
   the same component declare the same id). Observations inherit the enclosing
   operation's component; a `watch_dep` inherits its enclosing operation's
   component unless it declares an override, and grandchildren inherit the dep's
   effective component.
4. **Declared-error mapping.** Each native error / declared exception maps
   deterministically to a `conformance.error.name` — the structural discriminant
   (a `#[derive(Watchable)]` variant tag, else the error type's last path
   segment) — with `conformance.error.value` the decomposed value; an unmapped
   failure is a `conformance.fault`.
5. **NaN canonicalization.** Non-finite floats wire as OTLP `doubleValue` strings
   (`"NaN"`/`"Infinity"`/`"-Infinity"`, CTSC §8.4). OTLP does not preserve NaN
   payload bits, so the runtime normalizes every NaN to canonical `"NaN"` at the
   OTLP boundary — bitwise-distinct NaNs must not surface as false drift.
6. **Feature gating.** When tracing is disabled (Rust: `trace` feature off; C#:
   build symbol), annotations expand to identity; a purpose-built OTLP recorder is
   used — no OpenTelemetry SDK dependency.

## Comparison & registry

Default comparison policy: **CTSC Strict** (`ctsc.strict/0.1.0`) — sequential
operations pair by position. If real captures show unstable operation ordering
across versions, a Driftwatch custom policy may add input-keyed operation matching
(CTSC §8). `discover` generates the CTSC Registry document from the link-time
registry; Linked validation binds a trace to it.

## Change control

Adopting a new CTSC version, or changing any producer choice above, requires human
ratification and a corpus update.