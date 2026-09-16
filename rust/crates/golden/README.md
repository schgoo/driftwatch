# Golden

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch golden trace corpus: the reusable annotated fixture library.

Each byte-exact CTSC 0.2 golden under the repository-root `tests/golden/`
directory is produced by running **real annotated fixture code** — the
operations in this crate — through the annotation macros and runtime, then
serializing the drained span tree with the [`artifact`][__link0] emitter. The
integration test in `tests/golden.rs` is the harness: it frames each fixture
with a `conformance.run` + `conformance.scenario` span, invokes these
operations, pins the (otherwise nondeterministic) trace id to a fixed
constant, and byte-compares the emitted OTLP against the on-disk golden — or
regenerates it under `DW_BLESS=1`.

These operations are a **library** (not test-local) so the future extraction
driver can drive the identical code. Off-trace they expand to inert identity;
the goldens run only under the `trace` feature.

## Fixtures

* [`sequential`][__link1] — two operations in one scenario, one with an observation;
* [`outcomes`][__link2] — the four completion dispositions (result / empty / unit /
  error);
* [`values`][__link3] — every CTSC §8 value type and edge via `watch_point!`;
* [`dependency`][__link4] — an operation with a `watch_dep!` nested call;
* [`faults`][__link5] — a panicking operation and a panicking dependency cascade.

The supervisor fault and the streaming (`.jsonl`) golden are assembled by the
harness itself (no annotation produces a supervisor fault, and streaming
reuses two of the fixtures above).


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbYtij0IKBpnEba8cG9PGS5Yob0YQYbn405d8bgM3PyF2fSsthZIaCaGFydGlmYWN0ZTAuMS4wgmpkZXBlbmRlbmN59oJmZmF1bHRz9oJob3V0Y29tZXP2gmpzZXF1ZW50aWFs9oJmdmFsdWVz9g
 [__link0]: https://crates.io/crates/artifact/0.1.0
 [__link1]: https://crates.io/crates/sequential
 [__link2]: https://crates.io/crates/outcomes
 [__link3]: https://crates.io/crates/values
 [__link4]: https://crates.io/crates/dependency
 [__link5]: https://crates.io/crates/faults
