# Phase R — Static Resolver (rust-analyzer type inference)

> **Status:** ratified this session (human owner). Recorded here as the durable
> decision; execution tracked as PRs **R-a … R-e** (see
> [`../roadmap.md`](../roadmap.md) § *Phase R — Static Resolver*).
>
> **Feasibility spike:** [`../spikes/ra-extract/REPORT.md`](../spikes/ra-extract/REPORT.md)
> (verdict **GO, with constraints**).
> **Acceptance oracle:** `tests/golden/resolved-registry.json` +
> `rust/crates/golden/tests/resolver_pending.rs`
> (`resolver_reproduces_resolved_registry`, red-by-design `#[ignore]`).

## Context

Driftwatch's contract is extracted at build time. Today the type pipeline is a
**string** pipeline: `extract::classify_return` / `parse_type_ref` lower an
operation's written return type by parsing its textual form. That pipeline
cannot see through **type aliases**, so aliased error channels are lost:

- A local alias (`type ChargeResult<T> = Result<T, ChargeError>`) hides the
  `Result` shape, leaving a dangling `Named("ChargeResult")` with no error
  outcome.
- A foreign alias (`io::Result<i64>`, `anyhow::Result<i64>`) shows one visible
  generic arg, so the op looks **infallible** — the error channel is silently
  dropped, not merely dangling.
- A primitive alias (`type Millis = u64`) lowers to a dangling `Named("Millis")`.

The spike wired **rust-analyzer** (`ra_ap_*`) as a static resolver and proved
real type inference normalizes all of these: aliases resolve to their underlying
`Result<T, E>` / primitive, recovering the error outcomes the string pipeline
cannot. The verdict was **GO, with constraints** (RA API/version pinning is
brittle and heavy; proc-macro expansion hurts HIR join-back for attribute
macros; `watch_point!` / `watch_dep!` token-tree → expression mapping needs
dedicated design).

Phase R replaces the build-time **string** type pipeline with an RA-backed
static resolver, keeping annotations as *selection / aiming* only.

## Ratified gate decisions

### Gate #1 — placement

A new **Phase R** is inserted **after Phase 3**. It **supersedes the build-time
string type-resolution front-half of issue #10** (registry emission) — the
`extract::derive` string driver (`10a`), i.e. `extract::classify_return` /
`parse_type_ref`. The RA resolver produces already-resolved `contract` types
that feed `derive` directly; **#10's output contract and its byte-reproducibility
guarantees are unchanged** — only the type-resolution front-half is replaced, and
#10 is **not deleted**. Downstream consumers **#12–#16** (Phase 5 diff / Phase 6
CLI) operate on whatever contract `derive` emits and are **untouched** by Phase R.

### Gate #2 — dependency / toolchain gating

RA is **feature-gated under the same feature that gates tracing/extraction**
(the Rust `trace` / `driftwatch` extraction feature). When tracing is off,
`ra_ap_*` — and its **rustc-1.96 toolchain pin** — is **never pulled**. When the
feature is on, **RA runs on every extraction**: it is the **default extraction
path**, not an optional add-on.

### Gate #3 — `derive()` seam

`extract::derive()` is **redefined to accept already-resolved `contract` types**
(`TypeRef` / observations / dependencies) instead of stringified metadata, and is
**relocated to the registry/contract layer**, shedding the string parsers. The
spike confirmed this is *seam friction*, not a production-code change: the
current `derive()` re-parses stringified `TypeRef`s and cannot accept resolved
types, so calling it with resolved inputs returned violations. Executed in
**R-b**; recorded here as the ratified direction.

### Gate #4 — error `ty`

Errors stay **name-only**. `ErrorOutcome.ty` remains `None`. The resolver names a
recovered error after its **last path segment** (trace-contract producer-choice
#4). Foreign-error name canonicalization is **DEFERRED** — see below.

## Deferred: foreign-error name canonicalization (producer choice #4)

> Foreign-error name canonicalization (producer choice #4). Foreign errors
> currently collapse to their last path segment (`io::Error`, `anyhow::Error` →
> `"Error"`), deterministic only within a language. A language-agnostic
> cross-language error taxonomy (e.g., Rust `io::Error` + C# `IOException` →
> canonical `"IoError"`) is intentionally NOT attempted in v1. Revisiting
> requires a trace-contract change (human-owned per `docs/trace-contract.md:159`);
> out of scope for Phase R. The io+anyhow→`"Error"` collision in the acceptance
> harness (`tests/golden/resolved-registry.json`,
> `rust/crates/golden/tests/resolver_pending.rs`) is the pinned proof that
> foreign errors are an undisambiguated bucket by design.

## PR sequence (R-a … R-e)

| PR | Scope |
|---|---|
| **R-a** *(this record; doc-only)* | Track the spike into `docs/spikes/ra-extract/`; write this decision record; add the Phase-R roadmap stub + renumber/cross-reference notes. No crate/test/oracle changes. |
| **R-b** | Redefine `derive()` over resolved `contract` types (Gate #3), relocate it to the registry layer, adapt the legacy string path. Harness stays green. |
| **R-c** | New feature-gated resolver crate: pinned `ra_ap_*=0.0.349` + `unicode-ident=1.0.22`, `ProcMacroServerChoice::None` + source pre-scan; resolve a target to resolved observations. Adapt the spike PoC. |
| **R-d** | Wire the resolver into `extract` as the **default** path (runs every extraction, feature-gated, Gate #2); `watch_point!` / `watch_dep!` token→expr mapping (PARTIAL risks flagged in the spike). |
| **R-e** | Delete the red-by-design `#[ignore]` on `resolver_reproduces_resolved_registry`; retire the superseded string parsers. **The oracle flipping green is the acceptance gate.** |

## Pins & constraints carried from the spike

- `ra_ap_hir`, `ra_ap_ide`, `ra_ap_ide_db`, `ra_ap_load-cargo`,
  `ra_ap_project_model`, `ra_ap_syntax`, `ra_ap_vfs` all `=0.0.349` (latest
  `0.0.352` needs rustc 1.98; repo toolchain is rustc 1.96.0).
- Extra pin `unicode-ident = =1.0.22` (avoids a compile-time Unicode-version
  mismatch panic in `ra-ap-rustc_lexer`).
- `cargo_config.sysroot = Some(RustLibSource::Discover)` and
  `ProcMacroServerChoice::None` — with proc-macro expansion enabled,
  `Semantics::to_def` fails on `#[watch_operation]` items; selection must scan
  source attrs and resolve **pre-expansion** syntax.
- **PARTIAL** risks (production must design robustly): macro-token-tree →
  expression mapping for `watch_point!` / `watch_dep!`, and resolving the call
  target inside `watch_dep!` token trees with proc macros disabled.

## Out of scope (do not reopen without a new ratification)

- Any load-bearing artifact / canonicalization / CLI decision beyond the four
  gates above.
- Cross-language foreign-error canonicalization (deferred; trace-contract-owned).
- Errors carrying a `ty` (stays `None` in v1).
