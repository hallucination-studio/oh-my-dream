//! Immutable exact-capability implementation registry.

use std::{collections::BTreeMap, sync::Arc};

use super::{
    NodeCapabilityContract, NodeCapabilityContractRef, NodeCapabilityRegistryError,
    WorkflowNodeCapabilityInterface,
};

/// Immutable collection of active exact node-capability implementations.
pub struct WorkflowNodeCapabilityRegistry {
    implementations: BTreeMap<NodeCapabilityContractRef, Arc<dyn WorkflowNodeCapabilityInterface>>,
}

impl WorkflowNodeCapabilityRegistry {
    /// Builds a registry and rejects the first duplicate ref in input order.
    pub fn try_new(
        implementations: impl IntoIterator<Item = Arc<dyn WorkflowNodeCapabilityInterface>>,
    ) -> Result<Self, NodeCapabilityRegistryError> {
        let mut by_ref = BTreeMap::new();
        for implementation in implementations {
            let contract_ref = implementation.node_capability_contract().contract_ref().clone();
            if by_ref.insert(contract_ref.clone(), implementation).is_some() {
                return Err(NodeCapabilityRegistryError::DuplicateContractRef { contract_ref });
            }
        }
        Ok(Self { implementations: by_ref })
    }

    /// Lists the same contracts used for execution in ascending ref order.
    #[must_use]
    pub fn list_node_capability_contracts(&self) -> Vec<&NodeCapabilityContract> {
        self.implementations
            .values()
            .map(|implementation| implementation.node_capability_contract())
            .collect()
    }

    /// Resolves one exact shared implementation without version fallback.
    pub fn resolve_node_capability(
        &self,
        contract_ref: &NodeCapabilityContractRef,
    ) -> Result<Arc<dyn WorkflowNodeCapabilityInterface>, NodeCapabilityRegistryError> {
        self.implementations.get(contract_ref).cloned().ok_or_else(|| {
            NodeCapabilityRegistryError::ContractNotRegistered {
                contract_ref: contract_ref.clone(),
            }
        })
    }
}
