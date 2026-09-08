# Value canonicalization

Captured values encode as **nested structural `Value`**, never flattened. See
[`trace-contract.md`](trace-contract.md) §Values for the source-type → value
table.

## Ruling

- Structs → string-keyed `Value::Map`, one entry per `#[watchable]` field.
- A `&mut Config` input is captured one-piece under the parameter name — not
  split into dotted per-field lines.
- Representation-independent identity comes from **explicit naming**, not the
  encoding.

## Nested vs flattened

| | Nested (used) | Flattened (rejected) |
|---|---|---|
| `cfg: &mut Config` | `{ "cfg": { "level": 0, "mode": "fast" } }` | `"cfg.level": 0`, `"cfg.mode": "fast"` |
| Covers whole lattice | yes | structs only |
| Identity bound to | type + structure | synthesized name path |

## Why nested

**Whole-lattice.** `Value` spans scalars, `Variant`, `List`, `Set`, `Map`. Only
a struct of named fields can be flattened; a `Vec`, `BTreeSet`, tuple, or
`Variant` payload has no key to synthesize. Nesting is the one encoding covering
the whole lattice — flattening would need two encodings plus a boundary policy.

**No positional leak.** Flattening binds identity to a name path that reflects
plumbing, not the value. If v1 takes `"level": 0` and v2 moves `level` under a
`cfg` struct, flattening emits `"cfg.level": 0` — spurious `level` vs `cfg.level`
drift though `0` never changed. Nesting keeps `0` at a structural position;
renaming the carrier does not rewrite identity.

**Tamper resistance.** The threat model includes an AI editor hiding drift. A
malleable name path lets a behavior-preserving edit — wrap a value in a struct,
rename the carrier, split a field — shift it in the flat keyspace. Structural
identity cannot move without changing what the program computes.

**Structure is not a report concern.** Flattening's readability is a rendering
choice; the comparator can render a nested value flat. Discarding structure at
capture is lossy and irreversible.

## Escape hatch: explicit naming

When identity must survive a representation change, name it:

- `watch_point!("name", &expr)` — stable checkpoint name whether `expr` is
  `level` or `cfg.level`.
- `#[watchable(name = "…")]` — field trace name, decoupled from the Rust field.
- `#[watch_input("name")]` — parameter input key, decoupled from the parameter.

Identity is what you named, not how the value is spelled. Test:
`semantic_name_is_representation_independent` in `annotations/tests/point.rs`.

## Boundaries

- **Moved/renamed hints** are the comparator's job (#13 / #23), computed from two
  nested captures — not baked into the capture by flattening.
- **`#[watchable(flatten)]`** (future, opt-in): inline one field's structure into
  its parent map — e.g. a newtype wrapper — a local, explicit author choice, not
  a global flat keyspace. Out of scope until a concrete need appears.
