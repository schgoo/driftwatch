# Extract

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch extraction driver.

Resolves a target’s [`Binding`][__link0] (the capture recipe), generates a runner
that drives annotated operations, builds and runs it, collects keyed traces,
and discovers the contract from the link-time registry.

```rust
let binding = extract::Binding::parse(r#"commands = ["cargo test"]"#)
    .expect("valid binding");
assert_eq!(binding.commands, ["cargo test"]);
```


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbKOfHH7ki0UobHasz6jhEpyAbQU52WhHJUagb2bhApbQ3_XJhZIGCZ2V4dHJhY3RlMC4xLjA
 [__link0]: https://docs.rs/extract/0.1.0/extract/?search=Binding
