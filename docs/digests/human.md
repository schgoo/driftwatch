# Human digest — Driftwatch

Concise status for people. Update when milestone status, ownership, or risks
change.

## Status

- **Phase: planning.** Roadmap and agent scaffolding drafted; no code yet.
- Next: PR0 (preserve SpecGate #36 work) → PR1 (scaffold + agent loop + docs +
  `cargo evaluate` harness).

## What it is (one line)

Diff two captures of a codebase's contract + behavioral traces to catch
version-to-version drift — change detection, not correctness; resistant to AI
weakening because the oracle is the previous version's own behavior.

## Fastest path to something real

Contract-diff (cheap, un-gameable, no trace driver or C# needed):
PR1 → PR6 → PR10 → PR12 → PR14 → PR15.

## Open decisions (owner needed)

Artifact format · trace canonicalization rules · CLI surface · macro naming.

## Risks

- Trace nondeterminism (addresses, map/async order) → false drift if
  canonicalization is incomplete. Load-bearing, carried from SpecGate #36.
- Trace-diff is only meaningful with enough input coverage (needs the
  property/input-generation feeder — see roadmap Phase 3+ and SpecGate #30).
