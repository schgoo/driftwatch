# Annotations-Macros

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

Procedural macros for the Driftwatch annotation surface.

These macros mark the operations, dependencies, state, and inline
checkpoints of a target crate so that running it emits a behavioral trace.
With the `trace` feature enabled (the default via the `annotations` facade)
they expand into calls funneled through `::annotations::__rt` — user code
never references the `runtime` or `linkme` crates directly. Depend on the
`annotations` facade rather than this crate.

## Production gating (`trace` feature)

Every entry point is gated on the `trace` feature. **With `trace` on**, each
macro emits its full expansion (event emission, the `$crate`-funnel paths,
and link-time registry statics). **With `trace` off**, each macro expands to
identity, so the output carries ZERO `emit_*` calls, ZERO `linkme` registry
statics, and ZERO `__rt` references:

* `#[watch_operation]` → the annotated item unchanged (bar removing the inert
  `#[watch_input]` / `#[watch_dep]` helper attributes, which cannot stand
  alone on a param or a statement);
* `#[watch_input]` / `#[watch_dep]` → the annotated statement unchanged;
* `watch_point!(…)` → `()`;
* `#[derive(Watchable)]` → nothing (no impls, no registry statics).

## Layout exception

A `proc-macro` crate must declare its `#[proc_macro*]` entry points at the
crate root, so — unlike the workspace’s other crates — this `lib.rs` carries
the thin entry functions in addition to module declarations. The real
expansion logic lives in the concern-scoped sibling modules; each entry point
is a thin, feature-gated delegate.


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

