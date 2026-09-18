# Resolver

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch static resolver.

`resolver` uses [rust-analyzer][__link0] as a
library to statically resolve a target crate’s annotated operations and
`#[derive(Watchable)]` types into the **already-resolved** contract IR:
[`contract::ResolvedOperation`][__link1] / [`contract::ResolvedType`][__link2]. It is the
type-aware successor to the retired alias-blind string pipeline: real type
inference sees through `type` aliases and foreign `Result` aliases
(`io::Result`, `anyhow::Result`) that a string classifier cannot, recovering
the hidden error channels.

The output feeds straight into [`contract::assemble`][__link3]: each
[`Resolved::operations`][__link4] / [`Resolved::types`][__link5] entry carries its resolved
`component` tag, `is_setup` flag, and a fully alias-resolved
[`contract::Operation`][__link6] / [`contract::NamedType`][__link7]. Registry assembly (the
envelope, component grouping, sorting) stays in `contract`.

### Feature gate

rust-analyzer and its heavy, pinned dependencies live behind the crate’s
`ra` feature (default **off**), so they are absent from the default
workspace build. With `ra` off the crate still compiles — the API types are
present — and [`resolve`][__link8] returns [`ResolveError::Unavailable`][__link9]. With `ra`
on, resolution is real.

```rust
use resolver::{ResolveError, ResolveTarget, resolve};

let target = ResolveTarget::new("path/to/crate");
let resolved = match resolve(&target) {
    Ok(resolved) => resolved,
    // Without the `ra` feature, resolution is unavailable — nothing to
    // assemble; a `Load` error is likewise a recoverable outcome to report.
    Err(ResolveError::Unavailable) => return,
    Err(err) => {
        eprintln!("resolution failed: {err}");
        return;
    }
};
let _doc = contract::assemble(
    &resolved.operations,
    &resolved.types,
    "urn:ctsc:registry:example:1".to_string(),
    "1.0.0".to_string(),
);
```


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQby1wiTnSwlSkb0ta3h1xJjtMbgZHP9h_LDGUb8yyhoy3wGl5hZIKCaGNvbnRyYWN0ZTAuMS4wgmhyZXNvbHZlcmUwLjEuMA
 [__link0]: https://rust-analyzer.github.io/
 [__link1]: https://docs.rs/contract/0.1.0/contract/?search=ResolvedOperation
 [__link2]: https://docs.rs/contract/0.1.0/contract/?search=ResolvedType
 [__link3]: https://docs.rs/contract/0.1.0/contract/?search=assemble
 [__link4]: https://docs.rs/resolver/0.1.0/resolver/?search=Resolved::operations
 [__link5]: https://docs.rs/resolver/0.1.0/resolver/?search=Resolved::types
 [__link6]: https://docs.rs/contract/0.1.0/contract/?search=Operation
 [__link7]: https://docs.rs/contract/0.1.0/contract/?search=NamedType
 [__link8]: https://docs.rs/resolver/0.1.0/resolver/?search=resolve
 [__link9]: https://docs.rs/resolver/0.1.0/resolver/?search=ResolveError::Unavailable
