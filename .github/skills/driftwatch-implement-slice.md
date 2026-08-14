# Skill: driftwatch-implement-slice

Implement one approved, bounded slice end to end — code, tests, docs — and
validate with the harness.

## Rules

- Implement exactly the approved packet. Do not broaden scope or touch files
  outside the declared boundary. Preserve unrelated changes.
- **Lifts from SpecGate:** port idiomatically to Rust; drop the TDD/matcher
  residue (matcher, operator catalog, `expected:` cases). Keep the trust-anchor
  invariants:
  - Trace goldens assert on derived `Debug` — never `Value` `PartialEq` (loose:
    Int==Float, List==Set) and never the serde wire form (Set serializes like
    List; NaN not JSON-representable).
  - Preserve deterministic ordering (`BTreeMap`/`BTreeSet`) and canonical
    encoding.
- Add or move tests alongside the code. Keep the PR ≤~500 net LoC.
- New unsafe / FFI / build scripts / proc-macros need an explicit safety
  contract and focused tests. `unsafe_code` is forbidden by workspace lint.

## Validate before handoff

Run and pass all of:

```
cargo test --workspace
cargo evaluate
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check
```

Fix failures; never silence a lint or the evaluate gate to pass. Do not commit
or push unless explicitly authorized.

## Output

Diff summary + harness evidence.
