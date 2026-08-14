# Skill: driftwatch-plan-slice

Turn one roadmap PR (issue #1–#20) into a bounded, evidence-backed task packet.
Read-only.

## Steps

1. **Read context:** `docs/roadmap.md` (the PR's scope + dependencies),
   `docs/digests/llm.md` (invariants + decision boundaries), and the relevant
   source — in this repo or the SpecGate code being lifted.
2. **Confirm prerequisites:** the PRs this one depends on are merged/available.
   If not, stop and report the missing dependency.
3. **Define the packet:**
   - Observable outcome (what exists/works when done).
   - Exact files/crates in scope; boundaries that must not change.
   - Acceptance evidence: which harness commands must pass
     (`cargo test` + `cargo evaluate` + `clippy -D warnings` + `fmt --check`).
   - Target size (≤500 net LoC) and how to stay within it.
   - Human stop conditions.
4. **Surface load-bearing decisions** (artifact format, trace canonicalization
   rules, CLI surface, macro naming) that must be owned by a human before
   implementation. Do not resolve them.

## Output

A task packet the implementer can execute without re-deriving scope. Do not edit
files.
