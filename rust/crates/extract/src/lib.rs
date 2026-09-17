//! The Driftwatch extraction driver.
//!
//! Resolves bindings, generates a runner that drives annotated operations,
//! builds and runs it, collects keyed traces, and discovers the contract from
//! the link-time registry.
//!
//! The contract-derivation driver ([`derive`]) lowers the runtime discovery
//! metadata (`runtime::OpMeta`/`runtime::TypeMeta`) into a
//! [`contract::RegistryDocument`].

mod derive;
mod identity;
mod return_kind;
mod type_ref;

pub use derive::derive;
pub use identity::RegistryIdentity;
