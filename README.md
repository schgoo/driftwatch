# Driftwatch

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)

Capture a codebase's **contract** (static API shape, from the link-time
registry) plus **behavioral traces** (dynamic, from annotated code), and **diff
two captures** to catch version-to-version drift.

The guarantee is **change detection, not correctness**: *"these observable
behaviors changed between A and B — did you mean to?"* It is
AI-tamper-resistant because the oracle is *another version's own behavior* —
there is no hand-authored assertion for an AI to weaken.

Driftwatch is a clean-room reboot of SpecGate that keeps only its extraction
half (annotations → traces, registry → contract) and drops the TDD/matcher half.

## Status

**Planning / early scaffold.** See [`docs/roadmap.md`](docs/roadmap.md) for the
architecture, artifact format, and PR-by-PR migration plan, tracked as GitHub
issues [#1–#20](https://github.com/schgoo/driftwatch/issues).

## Layout

- `rust/` — the Rust workspace (crates in `rust/crates/`).
- `csharp/` — the C# emitter (weaver + runtime) for cross-language extraction.
- `docs/` — planning docs and the migration roadmap.
- [`AGENTS.md`](AGENTS.md) — agent entry point, code structure, style guidelines.

## Development

```
just check   # build + test + clippy + fmt + cargo evaluate (the full gate)
```

`cargo evaluate` is part of the harness — treat a failure like a clippy failure.

## License

[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE)
