//! The Driftwatch golden trace corpus: the reusable annotated fixture library.
//!
//! Each byte-exact CTSC 0.1 golden under the repository-root `tests/golden/`
//! directory is produced by running **real annotated fixture code** — the
//! operations in this crate — through the annotation macros and runtime, then
//! serializing the drained span tree with the [`artifact`] emitter. The
//! integration test in `tests/golden.rs` is the harness: it frames each fixture
//! with a `conformance.run` + `conformance.scenario` span, invokes these
//! operations, pins the (otherwise nondeterministic) trace id to a fixed
//! constant, and byte-compares the emitted OTLP against the on-disk golden — or
//! regenerates it under `DW_BLESS=1`.
//!
//! These operations are a **library** (not test-local) so the future extraction
//! driver can drive the identical code. Off-trace they expand to inert identity;
//! the goldens run only under the `trace` feature.
//!
//! # Fixtures
//!
//! - [`sequential`] — two operations in one scenario, one with an observation;
//! - [`outcomes`] — the four completion dispositions (result / empty / unit /
//!   error);
//! - [`values`] — every CTSC §8 value type and edge via `watch_point!`;
//! - [`dependency`] — an operation with a `watch_dep!` nested call;
//! - [`faults`] — a panicking operation and a panicking dependency cascade.
//!
//! The supervisor fault and the streaming (`.jsonl`) golden are assembled by the
//! harness itself (no annotation produces a supervisor fault, and streaming
//! reuses two of the fixtures above).

mod dependency;
mod faults;
mod outcomes;
mod sequential;
mod values;

pub use dependency::to_int;
pub use faults::{boom, cascade_outer};
pub use outcomes::{LookupError, add, first_even, log_only, lookup};
pub use sequential::{subtotal, with_tax};
pub use values::value_edges;
