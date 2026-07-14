//! Node registry: maps a node `type_id` to a factory that builds an instance
//! from its serialized parameters.

use crate::capability::{
    CapabilityRef, CapabilityRegistration, CapabilityRegistry, CapabilityRegistryError,
};
use crate::error::EngineError;
use crate::node::Node;
use std::collections::HashMap;

/// Parameters for a node as stored in the workflow JSON (`params` object).
pub type NodeParams = serde_json::Map<String, serde_json::Value>;

/// A factory that constructs a [`Node`] from its serialized parameters.
///
/// Returns a boxed error on invalid params; the caller attaches node context.
pub type NodeFactory =
    Box<dyn Fn(&NodeParams) -> Result<Box<dyn Node>, crate::node::NodeRunError> + Send + Sync>;

/// Registry of known node types, populated at startup by the `nodes` crate.
#[derive(Default)]
pub struct NodeRegistry {
    factories: HashMap<String, NodeFactory>,
    capabilities: CapabilityRegistry,
}

impl NodeRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a factory under `type_id`, replacing any previous entry.
    pub fn register(&mut self, type_id: impl Into<String>, factory: NodeFactory) {
        self.factories.insert(type_id.into(), factory);
    }

    /// Instantiates a node of `type_id` from `params`.
    ///
    /// `node_id` is used only to enrich errors with graph context.
    pub fn instantiate(
        &self,
        node_id: &str,
        type_id: &str,
        params: &NodeParams,
    ) -> Result<Box<dyn Node>, EngineError> {
        if let Some(factory) = self.factories.get(type_id) {
            return factory(params).map_err(|source| EngineError::NodeExecution {
                node_id: node_id.to_owned(),
                type_id: type_id.to_owned(),
                source,
            });
        }
        let reference =
            self.capabilities.current(type_id).ok_or_else(|| EngineError::UnknownNodeType {
                node_id: node_id.to_owned(),
                type_id: type_id.to_owned(),
            })?;
        self.instantiate_capability(node_id, &reference, params)
    }

    /// Registers one exact capability and marks it current for compatibility discovery.
    pub fn register_capability(
        &mut self,
        registration: CapabilityRegistration,
    ) -> Result<(), CapabilityRegistryError> {
        self.capabilities.register_current(registration)
    }

    /// Resolves an exact capability contract without selecting a newer version.
    pub fn capability(
        &self,
        reference: &CapabilityRef,
    ) -> Result<&CapabilityRegistration, CapabilityRegistryError> {
        self.capabilities.resolve(reference)
    }

    /// Returns all exact capability refs in stable order.
    #[must_use]
    pub fn capability_refs(&self) -> Vec<&CapabilityRef> {
        self.capabilities.references()
    }

    /// Returns current exact capability refs eligible for new-node discovery.
    #[must_use]
    pub fn current_capability_refs(&self) -> Vec<CapabilityRef> {
        self.capabilities.current_references()
    }

    /// Instantiates a Workflow node using its persisted exact contract version.
    pub fn instantiate_workflow_node(
        &self,
        node_id: &str,
        type_id: &str,
        contract_version: &str,
        params: &NodeParams,
    ) -> Result<Box<dyn Node>, EngineError> {
        if !self.capabilities.contains_id(type_id) {
            return self.instantiate(node_id, type_id, params);
        }
        let reference = CapabilityRef::new(type_id, contract_version);
        self.instantiate_capability(node_id, &reference, params)
    }

    /// Returns whether a `type_id` is registered.
    #[must_use]
    pub fn contains(&self, type_id: &str) -> bool {
        self.factories.contains_key(type_id) || self.capabilities.contains_id(type_id)
    }

    /// Returns registered node type ids in stable lexical order.
    #[must_use]
    pub fn registered_type_ids(&self) -> Vec<&str> {
        let mut type_ids = self
            .factories
            .keys()
            .map(String::as_str)
            .chain(self.capability_refs().into_iter().map(|reference| reference.id.as_str()))
            .collect::<Vec<_>>();
        type_ids.sort_unstable();
        type_ids.dedup();
        type_ids
    }

    fn instantiate_capability(
        &self,
        node_id: &str,
        reference: &CapabilityRef,
        params: &NodeParams,
    ) -> Result<Box<dyn Node>, EngineError> {
        let registration = self.capabilities.resolve(reference).map_err(|_| {
            EngineError::UnknownCapabilityVersion {
                node_id: node_id.to_owned(),
                type_id: reference.id.clone(),
                contract_version: reference.version.clone(),
            }
        })?;
        let normalized = registration.normalize_params(params).map_err(|source| {
            EngineError::InvalidCapabilityParams {
                node_id: node_id.to_owned(),
                type_id: reference.id.clone(),
                contract_version: reference.version.clone(),
                source,
            }
        })?;
        let node =
            registration.instantiate(&normalized).map_err(|source| EngineError::NodeExecution {
                node_id: node_id.to_owned(),
                type_id: reference.id.clone(),
                source,
            })?;
        validate_contract_ports(registration.contract(), node.as_ref())?;
        Ok(node)
    }
}

fn validate_contract_ports(
    contract: &crate::CapabilityContract,
    node: &dyn Node,
) -> Result<(), EngineError> {
    if node.inputs().len() != contract.inputs.len() {
        return Err(EngineError::CapabilityContractMismatch {
            type_id: contract.reference.id.clone(),
            message: "input port count differs from the contract".to_owned(),
        });
    }
    if node.outputs().len() != contract.outputs.len() {
        return Err(EngineError::CapabilityContractMismatch {
            type_id: contract.reference.id.clone(),
            message: "output port count differs from the contract".to_owned(),
        });
    }
    for port in &contract.inputs {
        let Some(actual) = node.input_port(&port.name) else {
            return Err(EngineError::CapabilityContractMismatch {
                type_id: contract.reference.id.clone(),
                message: format!("missing input port `{}`", port.name),
            });
        };
        if actual.port_type != port.port_type
            || actual.required != port.required
            || actual.cardinality != port.cardinality
        {
            return Err(EngineError::CapabilityContractMismatch {
                type_id: contract.reference.id.clone(),
                message: format!(
                    "input port `{}` has different type, cardinality, or requiredness",
                    port.name
                ),
            });
        }
    }
    for port in &contract.outputs {
        let Some(actual) = node.output_port(&port.name) else {
            return Err(EngineError::CapabilityContractMismatch {
                type_id: contract.reference.id.clone(),
                message: format!("missing output port `{}`", port.name),
            });
        };
        if actual.port_type != port.port_type {
            return Err(EngineError::CapabilityContractMismatch {
                type_id: contract.reference.id.clone(),
                message: format!("output port `{}` has a different type", port.name),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_type_ids_are_stable_and_sorted() {
        let mut registry = NodeRegistry::new();
        registry.register("Video", Box::new(|_| unreachable!()));
        registry.register("Audio", Box::new(|_| unreachable!()));

        assert_eq!(registry.registered_type_ids(), vec!["Audio", "Video"]);
    }
}
