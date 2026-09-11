# Artifact

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch artifact emitter: serialize an in-memory capture to CTSC OTLP.

The [`runtime`][__link0] crate builds an in-memory tree of `conformance.*` spans (see
its [`Span`][__link1]/[`SpanEvent`][__link2] types) but does
not persist them. This crate is that persistence boundary: it renders a
[`TraceCapture`][__link3] — a caller-supplied [`Resource`][__link4] descriptor plus the drained
`Vec<runtime::Span>` — into a **CTSC 0.1**-conformant OTLP `TracesData`
JSON document (see `docs/trace-contract.md`).

The recorder is purpose-built (CTSC §3 permits a hand-built producer); it
pulls in no OpenTelemetry SDK, only `serde_json` for JSON assembly.

## What is emitted

A single [`TraceCapture`][__link5] renders to one `TracesData` object:
`resourceSpans[0]` carries the [`Resource`][__link6] attributes and one `scopeSpans`
batch whose flat `spans` list is the capture’s span tree (nesting is carried
by `parentSpanId`, root spans omit it). Each span emits its ids as
lowercase-hex, its `start`/`end`/event ticks as decimal `*UnixNano` strings,
numeric `kind` (`1` = `SPAN_KIND_INTERNAL`) and `status.code`
(`Error` → `2`, `Unset`/`Ok` → `1` — OK-promotion is owned here), its
attributes, and its ordered events.

[`runtime::Value`][__link7] maps to OTLP `AnyValue` per CTSC §8: integers and
non-finite floats wire as strings (`intValue`, `doubleValue` `"NaN"` /
`"Infinity"` / `"-Infinity"`, with every NaN bit-pattern normalized to
`"NaN"`), a `Set` wires as a stable (already-sorted) `arrayValue` just like a
`List`, a `Map` as a `kvlistValue`, and a `Variant` as a single-key
`kvlistValue` tagged union.

## Formats

[`OtlpFormat::Json`][__link8] renders one pretty-printed `TracesData` object;
[`OtlpFormat::Jsonl`][__link9] renders the compact streaming form (one complete
`TracesData` per line, `\n`-terminated — the CTSC §2 OTLP File Exporter shape).

## Example

```rust
use artifact::{OtlpFormat, Resource, TraceCapture};

let resource = Resource::new("driftwatch", "0.1.0", "my-target", "rust");
let capture = TraceCapture {
    resource,
    spans: Vec::new(),
};
let json = capture.to_otlp(OtlpFormat::Json);
assert!(json.contains("\"resourceSpans\""));
```


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQboqibkVMHhDYbsIucyRmUcFMbuAQPX9MW8kEbDGbMhNPSaGBhZIKCaGFydGlmYWN0ZTAuMS4wgmdydW50aW1lZTAuMS4w
 [__link0]: https://crates.io/crates/runtime/0.1.0
 [__link1]: https://docs.rs/runtime/0.1.0/runtime/?search=Span
 [__link2]: https://docs.rs/runtime/0.1.0/runtime/?search=SpanEvent
 [__link3]: https://docs.rs/artifact/0.1.0/artifact/?search=TraceCapture
 [__link4]: https://docs.rs/artifact/0.1.0/artifact/?search=Resource
 [__link5]: https://docs.rs/artifact/0.1.0/artifact/?search=TraceCapture
 [__link6]: https://docs.rs/artifact/0.1.0/artifact/?search=Resource
 [__link7]: https://docs.rs/runtime/0.1.0/runtime/?search=Value
 [__link8]: https://docs.rs/artifact/0.1.0/artifact/?search=OtlpFormat::Json
 [__link9]: https://docs.rs/artifact/0.1.0/artifact/?search=OtlpFormat::Jsonl
