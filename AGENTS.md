# Driftwatch

Driftwatch captures a **contract** (static API shape, from the link-time
registry) plus **behavioral traces** (dynamic, from annotated code) out of a
codebase, and **diffs two captures** to catch version-to-version drift.

The guarantee is **change detection, not correctness**: "these observable
behaviors changed between A and B — did you mean to?" It is AI-tamper-resistant
because the oracle is *another version's own behavior* — there is no
hand-authored assertion for an AI to weaken. Origin: a clean-room reboot of
SpecGate keeping only its extraction half.

See [`docs/roadmap.md`](docs/roadmap.md) for the full architecture, artifact
format, salvage map, and PR-by-PR migration plan. **Read the roadmap first.**

## Code structure

All paths relative to the repository root.

- `rust/` — the Rust workspace. Crates live in `rust/crates/`.
  - `runtime` — the emitter: trace buffer, `Value` encoding,
    `SpecEvent`, link-time registry, JSONL record mode.
  - `annotations(-macros)` — the annotation surface.
  - `contract` — contract (`.contract.yaml`) parsing + validation.
  - `extract` — extraction driver: binding-resolve, codegen,
    build+run+collect, `discover`.
  - `diff` — contract-diff + trace-diff.
  - `cli` — `snapshot`, `compare`, and `--mode full/diff/pr`.
- `csharp/` — the C# emitter (weaver + runtime) for cross-language extraction.
- `docs/` — design docs and the migration roadmap.

## Style guidelines

- All crate dependencies are declared in `rust/Cargo.toml`; member crates use
  `{crate}.workspace = true`.
- Implementation lives in named, concern-scoped sibling files — never in
  `mod.rs`, `lib.rs`, or `main.rs`. A crate root and any `mod.rs` only declare
  modules and re-export public items (`mod value; pub use value::Value;`),
  keeping each public item reachable through exactly one path (private `mod` +
  `pub use`). Group by concern — roughly a primary type plus its impls per file
  — and do not let one file accumulate unrelated subsystems as a crate grows.
  (`cargo evaluate`'s `m_balanced_modules` checks the module *namespace*, not
  physical file size, so this is our own convention.)
- Avoid exposing a public item through more than one path.
- Forbid unsafe code where practical. New unsafe / FFI / build scripts /
  proc-macros require an explicit safety contract, focused tests, and human
  review.
- **`cargo evaluate` is part of the test harness.** The justfile `check` recipe
  and CI run `cargo evaluate` (deterministic + semantic lint packs) alongside
  `cargo test`, `clippy`, and `fmt --check`. Treat an evaluate failure like a
  clippy failure — do not merge past it. Rule exceptions go in `evaluate.toml`,
  not by silencing the harness.

## Planning and agent workflow

Before planning or changing Driftwatch work, read:

1. `docs/README.md` — the planning-doc index.
2. `docs/roadmap.md` — the migration plan and current phase.
3. `docs/digests/llm.md` — operational facts and decision boundaries for agents.
4. The relevant PR's scope and acceptance criteria in the roadmap.

Use `.github/agents/driftwatch-orchestrator.agent.md` as the default delivery
controller. It runs an **observe → plan → act → verify** loop (documented in
`docs/agentic-loop.md`) and delegates to tool-scoped specialist agents (planner,
implementer, reviewer) that apply the repository skills under `.github/skills`.
Verification includes running the harness (`cargo test` + `cargo evaluate` +
`clippy` + `fmt --check`). Human PR review remains the acceptance gate.

Do not silently finalize the load-bearing contracts — the artifact format, the
trace canonicalization rules, and the CLI surface — while implementing a bounded
PR. Those require an explicit decision owner.
