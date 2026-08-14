# Driftwatch planning docs

Shared planning context for humans and coding agents working on Driftwatch.

## Start here

- [Roadmap](roadmap.md) — architecture, artifact format, salvage map, and the
  PR-by-PR migration plan. **Read this first.**
- [Agentic loop](agentic-loop.md) — how the orchestrator observes, plans,
  delegates to specialists, and verifies before human PR review.
- [LLM digest](digests/llm.md) — operational facts and decision boundaries for
  agents.
- [Human digest](digests/human.md) — concise status, ownership, and risks.
- [References](references.md) — source project (SpecGate), tooling, and Copilot
  customization docs.
- [`../AGENTS.md`](../AGENTS.md) — project overview, code structure, and style
  guidelines (including `cargo evaluate` as a harness gate).

## Sources of truth

The current human directive and accepted decisions take precedence. This tree
summarizes the migration program; it does not replace the load-bearing
decisions (artifact format, trace canonicalization rules, CLI surface), which
need an explicit owner, or human PR acceptance.
