# Contract

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch contract document.

A Driftwatch contract is the static, verifiable API shape — components,
operations, inputs, observations, outcomes, and types. It *is* the **CTSC
Registry 0.2** document (`ctsc.registry`, JSON), specified normatively by
CTSC’s `registry.md` and `ctsc-registry-0.2.schema.json`; `discover`
generates it from the link-time registry.

This crate models that document, [parses][__link0] it from
JSON, and [validates][__link1] the cross-item semantic rules the JSON
schema cannot express (registry.md §4.1, §7, §8, §10).

```rust
let json = r#"{
  "format": "ctsc.registry",
  "formatVersion": "0.2.0",
  "registryId": "urn:ctsc:registry:example:1",
  "version": "1.0.0",
  "components": [
    { "id": "example", "operations": [], "types": [] }
  ]
}"#;
let doc = contract::RegistryDocument::parse(json).expect("parses");
assert!(contract::validate(&doc).is_empty());
```


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbOl1aayfruk4bG5kI4GiMXjYb-bDyXNc1tUEbKCkE01OUjr5hZIGCaGNvbnRyYWN0ZTAuMS4w
 [__link0]: https://docs.rs/contract/0.1.0/contract/?search=RegistryDocument::parse
 [__link1]: https://docs.rs/contract/0.1.0/contract/?search=validate
