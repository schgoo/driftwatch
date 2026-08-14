# References — Driftwatch

## Source project

- **SpecGate** (`~/repos/specgate`) — the codebase Driftwatch reboots. Lift the
  extraction half; leave the TDD/matcher half. Key context lives in SpecGate
  issues #36 (trace-engine trust anchor + goldens/encoder tests + F1/F2/F3
  canonicalization findings), #26 (differentiation from TDD / refactor oracle =
  the compare idea), and #30 (proptest inputs → trace diff).

## Tooling

- **`cargo evaluate`** — deterministic + AI-driven (semantic) lint packs,
  organized into reusable packs; part of the Driftwatch harness. Modes
  `full|diff|pr` (the model for the Driftwatch CLI's own `--mode`). Config in
  `evaluate.toml`; packs auto-discovered from `.evaluate/`.

## Conventions mirrored from `~/repos/dcf`

- `AGENTS.md` at the repo root as the agent entry point.
- `.github/agents/*.agent.md` orchestrator + specialist agents; `.github/skills`
  playbooks.
- `docs/` planning tree: README index, `agentic-loop.md`, `digests/{human,llm}.md`,
  `references.md`, and a delivery plan (here: `roadmap.md`).

## Copilot customization

- Custom agents are selected via `/agent`; they are not standalone slash
  commands. Specialist agents are `user-invocable: false`.
