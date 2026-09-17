//! The identity a derived registry document is stamped with (registry.md §2).

/// The `registryId`/`version` pair a derived [`contract::RegistryDocument`] is
/// stamped with.
///
/// This is a plain value carrier: resolving the identity from configuration or
/// the environment is a separate concern (roadmap #10b). Here it is simply a
/// parameter to [`crate::derive`].
#[derive(Debug, Clone)]
pub struct RegistryIdentity {
    /// The registry family id (§2), e.g. `"urn:ctsc:registry:example:1"`.
    pub registry_id: String,
    /// The source package/release/commit/build the document is derived from (§2).
    pub version: String,
}
