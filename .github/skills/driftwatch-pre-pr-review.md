# Skill: driftwatch-pre-pr-review

Independently verify a slice before it is offered for human PR review. Read-only
except for running the harness.

## Steps

1. **Inspect the diff** — do not trust the implementer's summary. Before running
   any repository-controlled code, inspect changes to manifests, lockfiles,
   build scripts, proc-macros, and task runners.
2. **Run the harness independently:**
   ```
   cargo test --workspace
   cargo evaluate
   cargo clippy --workspace --all-targets -- -D warnings
   cargo fmt -- --check
   ```
   An evaluate failure is blocking, like a clippy failure.
3. **Check acceptance:**
   - Scope respected; no unrelated changes; ≤~500 net LoC.
   - Trust-anchor invariants preserved (Debug oracle; no loose-equality or
     lossy-serde reliance; deterministic canonical encoding).
   - No load-bearing decision (artifact format, canonicalization, CLI surface,
     macro naming) silently finalized.
   - Tests cover the new/changed behavior; unsafe/build-script/macro changes
     carry a safety contract and focused tests.

## Output

Blocking findings with exact acceptance criteria, or a pass confirming the slice
is ready for human PR review. Do not edit files.
