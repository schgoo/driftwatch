# Extract

[![CI](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/schgoo/driftwatch/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)

The Driftwatch extraction driver.

Resolves bindings, generates a runner that drives annotated operations,
builds and runs it, collects keyed traces, and discovers the contract from
the link-time registry.

The contract-derivation driver ([`derive`][__link0]) lowers the runtime discovery
metadata (`runtime::OpMeta`/`runtime::TypeMeta`) into a
[`contract::RegistryDocument`][__link1].


---

Part of the [Driftwatch](https://github.com/schgoo/driftwatch) project.

 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjNhdIQbczlzGuhUQj4bPuh9UW2lL-EbW470-h7a1-0bxL56aHOBGtZhYvRhcoQbUVXtkifyeuIbmD6NOQ8xVhgbr63VxYS43VYbd-QwSDg1NTdhZIKCaGNvbnRyYWN0ZTAuMS4wgmdleHRyYWN0ZTAuMS4w
 [__link0]: https://docs.rs/extract/0.1.0/extract/?search=derive
 [__link1]: https://docs.rs/contract/0.1.0/contract/?search=RegistryDocument
