//! Assemble a [`RegistryDocument`] from a resolved intermediate representation
//! (registry.md §2–§5).
//!
//! [`assemble`] is the back-half of registry derivation: it operates purely on
//! contract types — [`Operation`]/[`NamedType`] already lowered from their
//! source form — grouping them into components, sorting for byte-reproducible
//! output, and wrapping the result in the [`RegistryDocument`] envelope.
//!
//! Keeping this seam over a resolved IR ([`ResolvedOperation`]/[`ResolvedType`])
//! lets a static resolver feed already-resolved contract types straight into
//! assembly without routing back through the string parsers. The front-half
//! (string lowering into [`Operation`]/[`NamedType`]) lives in `extract`.
//!
//! ## Component grouping
//!
//! Operations and types are grouped by their resolved `component` tag, emitting
//! one [`Component`] per distinct tag under `components[]`. Components are
//! sorted by `id`, and within each component operations and types are sorted by
//! `name`, so the assembled document is byte-reproducible regardless of the
//! (platform-dependent) input order. Each component gets an empty `dependencies`
//! list. Setups (`ResolvedOperation::is_setup`) are wiring helpers, not contract
//! operations, so they are excluded from the operations *and* from component
//! grouping.

use std::collections::BTreeSet;

use crate::component::Component;
use crate::document::{FORMAT, FORMAT_VERSION, RegistryDocument};
use crate::named_type::NamedType;
use crate::operation::Operation;

/// The component id used only when the registry carries no operations or types,
/// so the document still declares the CTSC-required one-or-more components.
const DEFAULT_COMPONENT: &str = "default";

/// A resolved operation: a contract [`Operation`] tagged with the component it
/// belongs to and whether it is a setup (a wiring helper, not a contract
/// operation).
#[derive(Debug, Clone)]
pub struct ResolvedOperation {
    /// The component tag this operation is grouped under.
    pub component: String,
    /// Whether this operation is a setup (excluded from the assembled output).
    pub is_setup: bool,
    /// The lowered contract operation.
    pub operation: Operation,
}

/// A resolved named type: a contract [`NamedType`] tagged with the component it
/// belongs to.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    /// The component tag this type is grouped under.
    pub component: String,
    /// The lowered contract named type.
    pub named_type: NamedType,
}

/// Assemble a CTSC registry document from resolved operations and types.
///
/// `registry_id`/`version` stamp the document's identity (§2); they come solely
/// from the caller. Setups are excluded from the operations and from component
/// grouping.
#[must_use]
pub fn assemble(
    ops: &[ResolvedOperation],
    types: &[ResolvedType],
    registry_id: String,
    version: String,
) -> RegistryDocument {
    let mut components: Vec<Component> = component_ids(ops, types)
        .into_iter()
        .map(|cid| assemble_component(cid, ops, types))
        .collect();
    if components.is_empty() {
        components.push(Component {
            id: DEFAULT_COMPONENT.to_string(),
            description: None,
            dependencies: Vec::new(),
            operations: Vec::new(),
            types: Vec::new(),
        });
    }

    RegistryDocument {
        format: FORMAT.to_string(),
        format_version: FORMAT_VERSION.to_string(),
        registry_id,
        version,
        description: None,
        imports: Vec::new(),
        components,
    }
}

/// The distinct component tags across the (non-setup) operations and types,
/// sorted by tag so the assembled document is order-stable regardless of the
/// input order.
fn component_ids<'a>(ops: &'a [ResolvedOperation], types: &'a [ResolvedType]) -> Vec<&'a str> {
    ops.iter()
        .filter(|op| !op.is_setup)
        .map(|op| op.component.as_str())
        .chain(types.iter().map(|t| t.component.as_str()))
        .collect::<BTreeSet<&str>>()
        .into_iter()
        .collect()
}

/// Build the [`Component`] for one component tag from the resolved operations
/// and types that carry it.
fn assemble_component(cid: &str, ops: &[ResolvedOperation], types: &[ResolvedType]) -> Component {
    let mut operations: Vec<Operation> = ops
        .iter()
        .filter(|op| !op.is_setup && op.component == cid)
        .map(|op| op.operation.clone())
        .collect();
    operations.sort_by(|a, b| a.name.cmp(&b.name));

    let mut named_types: Vec<NamedType> = types
        .iter()
        .filter(|t| t.component == cid)
        .map(|t| t.named_type.clone())
        .collect();
    named_types.sort_by(|a, b| a.name().cmp(b.name()));

    Component {
        id: cid.to_string(),
        description: None,
        dependencies: Vec::new(),
        operations,
        types: named_types,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Outcomes;

    fn op(name: &str, component: &str, is_setup: bool) -> ResolvedOperation {
        ResolvedOperation {
            component: component.to_string(),
            is_setup,
            operation: Operation {
                name: name.to_string(),
                description: None,
                inputs: Vec::new(),
                observations: Vec::new(),
                outcomes: Outcomes::default(),
            },
        }
    }

    fn ty(name: &str, component: &str) -> ResolvedType {
        ResolvedType {
            component: component.to_string(),
            named_type: NamedType::Record {
                name: name.to_string(),
                fields: Vec::new(),
                description: None,
            },
        }
    }

    #[test]
    fn groups_and_sorts_components_operations_and_types() {
        // Deliberately out of order: components mail then billing, ops zulu then
        // alpha, types zeta then alpha.
        let ops = [
            op("notify", "mail", false),
            op("zulu", "billing", false),
            op("alpha", "billing", false),
        ];
        let types = [ty("Zeta", "billing"), ty("Alpha", "billing")];

        let doc = assemble(
            &ops,
            &types,
            "urn:ctsc:registry:example:1".to_string(),
            "1.0.0".to_string(),
        );

        let ids: Vec<&str> = doc.components.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["billing", "mail"]);

        let billing = &doc.components[0];
        let op_names: Vec<&str> = billing.operations.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(op_names, ["alpha", "zulu"]);
        let type_names: Vec<&str> = billing.types.iter().map(NamedType::name).collect();
        assert_eq!(type_names, ["Alpha", "Zeta"]);

        let mail = &doc.components[1];
        assert_eq!(mail.operations.len(), 1);
        assert_eq!(mail.operations[0].name, "notify");
        assert!(mail.types.is_empty());

        for component in &doc.components {
            assert!(component.dependencies.is_empty());
        }
    }

    #[test]
    fn setups_are_excluded_from_operations_and_grouping() {
        let ops = [
            op("charge", "billing", false),
            // A setup in its own would-be component must not create a component
            // and must not surface as an operation.
            op("seed", "fixtures", true),
        ];

        let doc = assemble(&ops, &[], "rid".to_string(), "v".to_string());

        let ids: Vec<&str> = doc.components.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["billing"]);
        let op_names: Vec<&str> = doc.components[0]
            .operations
            .iter()
            .map(|o| o.name.as_str())
            .collect();
        assert_eq!(op_names, ["charge"]);
    }

    #[test]
    fn empty_input_yields_default_component() {
        let doc = assemble(&[], &[], "rid".to_string(), "v".to_string());
        assert_eq!(doc.components.len(), 1);
        assert_eq!(doc.components[0].id, DEFAULT_COMPONENT);
        assert!(doc.components[0].operations.is_empty());
        assert!(doc.components[0].types.is_empty());
    }

    #[test]
    fn stamps_format_and_identity() {
        let doc = assemble(
            &[],
            &[],
            "urn:ctsc:registry:example:1".to_string(),
            "9.9.9".to_string(),
        );
        assert_eq!(doc.format, FORMAT);
        assert_eq!(doc.format_version, FORMAT_VERSION);
        assert_eq!(doc.registry_id, "urn:ctsc:registry:example:1");
        assert_eq!(doc.version, "9.9.9");
        assert!(doc.imports.is_empty());
    }
}
