# Annotations

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The extraction-time annotation surface for Driftwatch.

These macros are applied to a *target crate* while it runs under a Driftwatch
snapshot: annotating its operations, state, and inline checkpoints makes the
running code emit a CTSC [`Span`][__link0] tree into the runtime’s thread-local buffer
(drained via [`take_spans`][__link1]), and registers the annotated operations and
types into the link-time discovery registry. Annotated code never names the
`runtime` or `linkme` crates directly — the macros funnel every reference
through the hidden [`__rt`][__link2] re-export, so this facade is the single dependency
a target crate adds.

This is the *extraction surface*, not a stabilized public API end users are
meant to build applications against yet; nothing prevents such use, but the
shape is driven by the extraction driver’s needs and may change.

## Public surface

* Macros: [`watch_operation`][__link3], [`watch_dep`][__link4], [`watch_point`][__link5],
  [`watch_input`][__link6], and the [`Watchable`][__link7] derive.
* Runtime items users interact with directly: [`Value`][__link8], [`ToValue`][__link9],
  [`Span`][__link10], [`SpanEvent`][__link11], [`SpanName`][__link12], [`EventName`][__link13], [`take_spans`][__link14], and
  [`reset`][__link15].

## Production gating

Tracing is **off by default**: a plain build expands every macro to identity,
so the annotated items compile unchanged and emit nothing, with no registry
statics and no references to the runtime — production pays nothing. Building
(or depending) with the `trace` feature forwards to `annotations-macros/trace`
and turns on emission plus the link-time registry; this is exactly what the
extraction driver does while taking a snapshot.


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbQdO9KBckUoMbZRPz8zMZM9MbSuYnEZH3xjQbZ0dRY3sNav1hZIOCa2Fubm90YXRpb25zZTAuMS4wg3Jhbm5vdGF0aW9ucy1tYWNyb3NlMC4xLjByYW5ub3RhdGlvbnNfbWFjcm9zgmdydW50aW1lZTAuMS4w
 [__link0]: https://docs.rs/runtime/0.1.0/runtime/?search=Span
 [__link1]: https://docs.rs/runtime/0.1.0/runtime/?search=take_spans
 [__link10]: https://docs.rs/runtime/0.1.0/runtime/?search=Span
 [__link11]: https://docs.rs/runtime/0.1.0/runtime/?search=SpanEvent
 [__link12]: https://docs.rs/runtime/0.1.0/runtime/?search=SpanName
 [__link13]: https://docs.rs/runtime/0.1.0/runtime/?search=EventName
 [__link14]: https://docs.rs/runtime/0.1.0/runtime/?search=take_spans
 [__link15]: https://docs.rs/runtime/0.1.0/runtime/?search=reset
 [__link2]: https://docs.rs/annotations/0.1.0/annotations/__rt/index.html
 [__link3]: https://docs.rs/annotations-macros/0.1.0/annotations_macros/?search=watch_operation
 [__link4]: https://docs.rs/annotations-macros/0.1.0/annotations_macros/?search=watch_dep
 [__link5]: https://docs.rs/annotations-macros/0.1.0/annotations_macros/?search=watch_point
 [__link6]: https://docs.rs/annotations-macros/0.1.0/annotations_macros/?search=watch_input
 [__link7]: macro@Watchable
 [__link8]: https://docs.rs/runtime/0.1.0/runtime/?search=Value
 [__link9]: https://docs.rs/runtime/0.1.0/runtime/?search=ToValue
