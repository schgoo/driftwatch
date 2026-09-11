//! The public capture type and the OTLP rendering entry points.

use runtime::Span;

use crate::otlp;
use crate::resource::Resource;

/// One in-memory capture: the caller-supplied [`Resource`] descriptor plus the
/// drained span tree from the runtime buffer ([`runtime::take_spans`]).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_structs,
    reason = "a caller-constructed capture; both fields are part of the stable surface"
)]
pub struct TraceCapture {
    /// The CTSC resource attributes (tool + target identity).
    pub resource: Resource,
    /// The capture's spans, in open order; nesting is carried by
    /// `parent_span_id`.
    pub spans: Vec<Span>,
}

/// The OTLP serialization shape to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OtlpFormat {
    /// One pretty-printed `TracesData` JSON object (`.otlp.json`).
    Json,
    /// The streaming form: one complete `TracesData` per `\n`-terminated line
    /// (`.otlp.jsonl`, the CTSC §2 OTLP File Exporter shape).
    Jsonl,
}

impl TraceCapture {
    /// Renders this capture as a CTSC-conformant OTLP `TracesData` document in
    /// the requested [`OtlpFormat`].
    ///
    /// # Panics
    ///
    /// Never in practice: the only fallible step is serializing an owned
    /// `serde_json::Value`, which cannot fail (no custom `Serialize` is involved).
    #[must_use]
    pub fn to_otlp(&self, format: OtlpFormat) -> String {
        let traces = otlp::traces_data(&self.resource, &self.spans);
        match format {
            OtlpFormat::Json => serde_json::to_string_pretty(&traces)
                .expect("serializing a serde_json::Value is infallible"),
            OtlpFormat::Jsonl => {
                let mut line = serde_json::to_string(&traces)
                    .expect("serializing a serde_json::Value is infallible");
                line.push('\n');
                line
            }
        }
    }
}
