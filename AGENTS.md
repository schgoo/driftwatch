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
  - `driftwatch-runtime` — the emitter: trace buffer, `Value` encoding,
    `SpecEvent`, link-time registry, JSONL record mode.
  - `driftwatch-annotations(-macros)` — the annotation surface.
  - `driftwatch-contract` — contract (`.contract.yaml`) parsing + validation.
  - `driftwatch-extract` — extraction driver: binding-resolve, codegen,
    build+run+collect, `discover`.
  - `driftwatch-diff` — contract-diff + trace-diff.
  - `driftwatch-cli` — `snapshot`, `compare`, and `--mode full/diff/pr`.
- `csharp/` — the C# emitter (weaver + runtime) for cross-language extraction.
- `docs/` — design docs and the migration roadmap.

## Style guidelines

- All crate dependencies are declared in `rust/Cargo.toml`; member crates use
  `{crate}.workspace = true`.
- Implementation code does not live in `mod.rs`; `mod.rs` only references and
  re-exports sibling files.
- Avoid exposing a public item through more than one path.
- Forbid unsafe code where practical. New unsafe / FFI / build scripts /
  proc-macros require an explicit safety contract, focused tests, and human
  review.

## Planning and agent workflow

Before planning or changing Driftwatch work, read:

1. `docs/roadmap.md` — the migration plan and current phase.
2. The relevant PR's scope and acceptance criteria in the roadmap.

Use `.github/agents/driftwatch-orchestrator.agent.md` as the default delivery
controller. It runs an **observe → plan → act → verify** loop and delegates to
tool-scoped specialist agents (planner, implementer, reviewer) that apply the
repository skills under `.github/skills`. Human PR review remains the
acceptance gate.

Do not silently finalize the load-bearing contracts — the artifact format, the
trace canonicalization rules, and the CLI surface — while implementing a bounded
PR. Those require an explicit decision owner.
