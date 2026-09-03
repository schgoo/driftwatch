# Runtime

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch runtime: the low-level substrate that annotated code emits
behavioral events into.

Driftwatch captures a program’s observable behavior as a stream of watch
events and diffs two captures to detect version-to-version drift. This crate
is the bottom of that stack — the structured value type, the event record,
and the conversion trait the annotation macros expand into. The extraction
driver runs annotated code and collects what this crate emits; the diff
engine compares two such collections. Nothing here knows about diffing or
bindings; it only defines *what an event is made of*.

## Key types

* [`Value`][__link0] — the universal structured value: any scalar, a tagged
  [`Value::Variant`][__link1], or a `list`/`map`/`set` of values. All integer widths
  canonicalize to `i64` and all float widths to `f64`.
* [`Span`][__link2] / [`SpanEvent`][__link3] — the OTLP-shaped CTSC span tree: a
  `conformance.operation` span carrying ordered completion/observation
  events (see [`span`][__link4]).
* [`ToValue`][__link5] — converts an annotated Rust value into a [`Value`][__link6]; this is
  the conversion the emit macros invoke.

## Emission and discovery

On top of the value type this crate provides the machinery the annotation
macros expand into: the thread-local span buffer and current-span stack
([`open_operation`][__link7], [`push_observation`][__link8], [`push_result`][__link9], [`push_empty`][__link10],
[`push_error`][__link11], [`take_spans`][__link12], [`reset`][__link13]), the [`ValueEmit`][__link14]
autoref-specialization ladder — the single value-encoding ladder shared by
the `conformance.result` and `conformance.error` dispositions — plus
[`split_error`][__link15] for the error name/value split, and the link-time
operation/type registry ([`OpMeta`][__link16] /
[`TypeMeta`][__link17] via [`discovery_json`][__link18]) the extraction driver reads to derive a
contract. The buffer holds in-memory spans only — persisting a capture is
the artifact layer’s job (see below).

## Comparison

[`Value`][__link19] implements strict, structural equality and a matching total order.
Two values are equal only when they have the same variant and the same
contents — an `Integer` never equals a `Float`, nor a `List` a `Set` — and
floats compare by their total ordering, so `NaN` equals itself and `-0.0`
differs from `0.0`. `eq` is defined as `cmp(..) == Equal`, so equality and
ordering can never disagree. Because equality is exact, “equal” means an
identical observation and any inequality is a genuine difference — the
property a drift check depends on.

Equality and ordering are not decorative: [`Span`][__link20]/[`SpanEvent`][__link21] compare by
value, and the `Set` variant is stored in a `BTreeSet`, which requires
`Value` to be totally ordered.

This crate defines only the in-memory span/value types and their comparison;
it does not serialize them. Persisting a capture to a file is the artifact
layer’s job, and its format must round-trip these values faithfully —
notably preserving `Set` vs `List` and the float edge cases — so that a
re-read capture compares identically to the original. Relaxed, field-scoped
matching for nondeterministic data (timestamps, random ids) likewise belongs
in the comparison layer above this crate, never in `Value`’s own equality.

## Examples

```rust
use runtime::{ToValue, Value};

// Convert a Rust value into the canonical value...
assert_eq!(
    vec![1_i32, 2].to_value(),
    Value::List(vec![Value::Integer(1), Value::Integer(2)])
);

// ...an `Option` projects to a tagged union.
assert_eq!(Some(5_i32).to_value(), Value::variant("Some", Value::Integer(5)));
```


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbozTuAkhZtOMbUB3rTx7GQpgbzNsd0iN8bHcb5jt0YM1PfBdhZIKCZ3J1bnRpbWVlMC4xLjCCZHNwYW72
 [__link0]: https://docs.rs/runtime/0.1.0/runtime/?search=Value
 [__link1]: https://docs.rs/runtime/0.1.0/runtime/?search=Value::Variant
 [__link10]: https://docs.rs/runtime/0.1.0/runtime/?search=push_empty
 [__link11]: https://docs.rs/runtime/0.1.0/runtime/?search=push_error
 [__link12]: https://docs.rs/runtime/0.1.0/runtime/?search=take_spans
 [__link13]: https://docs.rs/runtime/0.1.0/runtime/?search=reset
 [__link14]: https://docs.rs/runtime/0.1.0/runtime/?search=ValueEmit
 [__link15]: https://docs.rs/runtime/0.1.0/runtime/?search=split_error
 [__link16]: https://docs.rs/runtime/0.1.0/runtime/?search=OpMeta
 [__link17]: https://docs.rs/runtime/0.1.0/runtime/?search=TypeMeta
 [__link18]: https://docs.rs/runtime/0.1.0/runtime/?search=discovery_json
 [__link19]: https://docs.rs/runtime/0.1.0/runtime/?search=Value
 [__link2]: https://docs.rs/runtime/0.1.0/runtime/?search=Span
 [__link20]: https://docs.rs/runtime/0.1.0/runtime/?search=Span
 [__link21]: https://docs.rs/runtime/0.1.0/runtime/?search=SpanEvent
 [__link3]: https://docs.rs/runtime/0.1.0/runtime/?search=SpanEvent
 [__link4]: https://crates.io/crates/span
 [__link5]: https://docs.rs/runtime/0.1.0/runtime/?search=ToValue
 [__link6]: https://docs.rs/runtime/0.1.0/runtime/?search=Value
 [__link7]: https://docs.rs/runtime/0.1.0/runtime/?search=open_operation
 [__link8]: https://docs.rs/runtime/0.1.0/runtime/?search=push_observation
 [__link9]: https://docs.rs/runtime/0.1.0/runtime/?search=push_result
