# Driftwatch development loop

## Purpose

Use a controller agent to shorten research, implementation, and validation
cycles without moving architecture or acceptance authority away from humans.
Human PR review remains the approval gate; the loop prepares higher-quality
changes before reviewers accept them.

## Architecture

`driftwatch-orchestrator` is the default entry point and owns the state
machine. It observes repository state, plans one bounded next action, launches a
tool-scoped specialist agent, verifies the result through an independent
reviewer, and repeats. Skills are reusable playbooks under `.github/skills`;
each specialist agent loads and applies one skill.

| Specialist agent | Skill | Tool posture | Primary output |
|---|---|---|---|
| `driftwatch-planner` | `driftwatch-plan-slice` | Read-only research | Bounded PR packet + stop conditions |
| `driftwatch-implementer` | `driftwatch-implement-slice` | Read, edit, validate | Vertical slice + evidence |
| `driftwatch-reviewer` | `driftwatch-pre-pr-review` | Read-only verification | Blocking findings |

The planner is tool-enforced read-only. The reviewer is behaviorally read-only
(retains shell access for validation only).

## Control loop

1. **Observe** — reconcile the directive, roadmap phase, branch diff, and
   validation evidence. Treat prior agent claims as unverified until supported
   by repository or tool evidence.
2. **Plan** — choose one bounded PR from `roadmap.md` (target ≤500 net LoC),
   its acceptance evidence, and human stop conditions.
3. **Act** — invoke the specialist with complete context; require it to perform
   the work, not advise. Only one mutating child runs at a time.
4. **Verify** — after every mutation, delegate to the reviewer, which inspects
   the diff independently and runs the harness:
   `cargo test` + `cargo evaluate` + `cargo clippy -- -D warnings` +
   `cargo fmt --check`. An evaluate failure blocks like a clippy failure.
5. **Stop or repeat** — stop at completion, an unresolved human decision, a
   repeated failure that needs replanning, or an external blocker.

## Launching from GitHub Copilot CLI

Run `/agent`, then select **Driftwatch Orchestrator**. Specialist agents are
`user-invocable: false` and launched only as subagents. If the profile was
added after the session started, run `/restart`, then `/agent` again.

## Verification harness

Every PR must pass the full harness before it is offered for human review:

- `cargo test --workspace` — includes the trace goldens and encoder edge tests
  (the extraction trust anchor).
- `cargo evaluate` — deterministic + semantic lint packs; exceptions live in
  `evaluate.toml`, never silenced ad hoc.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt -- --check`.

## Load-bearing decisions (need an explicit owner)

Do not silently finalize these while implementing a bounded PR: the **artifact
format**, the **trace canonicalization rules** (Set≠List, NaN/Inf, no loose
equality), and the **CLI surface**.
