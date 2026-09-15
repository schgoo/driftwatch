//! The Driftwatch contract document.
//!
//! A Driftwatch contract is the static, verifiable API shape — components,
//! operations, inputs, observations, outcomes, and types. It *is* the **CTSC
//! Registry 0.2** document (`ctsc.registry`, JSON), specified normatively by
//! CTSC's `registry.md` and `ctsc-registry-0.2.schema.json`; `discover`
//! generates it from the link-time registry.
//!
//! This crate models that document, [parses](RegistryDocument::parse) it from
//! JSON, and [validates](validate) the cross-item semantic rules the JSON
//! schema cannot express (registry.md §4.1, §7, §8, §10).
//!
//! ```
//! let json = r#"{
//!   "format": "ctsc.registry",
//!   "formatVersion": "0.2.0",
//!   "registryId": "urn:ctsc:registry:example:1",
//!   "version": "1.0.0",
//!   "components": [
//!     { "id": "example", "operations": [], "types": [] }
//!   ]
//! }"#;
//! let doc = contract::RegistryDocument::parse(json).expect("parses");
//! assert!(contract::validate(&doc).is_empty());
//! ```

mod component;
mod document;
mod named_type;
mod operation;
mod type_ref;
mod validate;

pub use component::{Component, Dependency, Import};
pub use document::{FORMAT, FORMAT_VERSION, ParseError, RegistryDocument};
pub use named_type::NamedType;
pub use operation::{ErrorOutcome, NamedValue, Operation, Outcomes};
pub use type_ref::{Field, Primitive, TypeRef, Variant};
pub use validate::{Violation, ViolationKind, validate};
