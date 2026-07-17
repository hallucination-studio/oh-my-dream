use std::sync::Arc;
use std::time::Instant;

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractRef, WorkflowNodeCapabilityRegistry,
};

use super::{
    GenerationProfileAvailabilityObservation, GenerationProfileAvailabilityReaderInterface,
    GenerationProfileAvailabilityRequest, GenerationProfileCatalog, GenerationProfileDefinition,
    GenerationProfileError,
};

/// Lists exact active Workflow capability contracts.
pub struct NodeCapabilityListUseCase {
    registry: Arc<WorkflowNodeCapabilityRegistry>,
}

impl NodeCapabilityListUseCase {
    /// Wires the authoritative immutable capability registry.
    #[must_use]
    pub fn new(registry: Arc<WorkflowNodeCapabilityRegistry>) -> Self {
        Self { registry }
    }
    /// Returns owned contracts in exact registry order.
    #[must_use]
    pub fn list_node_capabilities(&self) -> Vec<NodeCapabilityContract> {
        self.registry.list_node_capability_contracts().into_iter().cloned().collect()
    }
}

/// Exact capability and deadline for selectable-profile listing.
pub struct GenerationProfileListForCapabilityQuery {
    capability_ref: NodeCapabilityContractRef,
    deadline: Instant,
}

impl GenerationProfileListForCapabilityQuery {
    /// Creates one exact capability-scoped query.
    #[must_use]
    pub const fn new(capability_ref: NodeCapabilityContractRef, deadline: Instant) -> Self {
        Self { capability_ref, deadline }
    }
    /// Returns the exact registered capability ref.
    #[must_use]
    pub const fn capability_ref(&self) -> &NodeCapabilityContractRef {
        &self.capability_ref
    }
    /// Returns the caller's monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
}

/// One selectable profile definition and matching current availability.
pub struct GenerationProfileForCapabilityListItem {
    definition: GenerationProfileDefinition,
    availability: GenerationProfileAvailabilityObservation,
}

impl GenerationProfileForCapabilityListItem {
    /// Returns the complete provider-independent profile definition.
    #[must_use]
    pub const fn definition(&self) -> &GenerationProfileDefinition {
        &self.definition
    }
    /// Returns its matching current availability observation.
    #[must_use]
    pub const fn availability(&self) -> &GenerationProfileAvailabilityObservation {
        &self.availability
    }
}

/// Joins exact catalog compatibility with one current bulk availability read.
pub struct GenerationProfileListForCapabilityUseCase {
    registry: Arc<WorkflowNodeCapabilityRegistry>,
    catalog: Arc<GenerationProfileCatalog>,
    availability_reader: Arc<dyn GenerationProfileAvailabilityReaderInterface>,
}

impl GenerationProfileListForCapabilityUseCase {
    /// Wires the registry, immutable catalog, and operational reader.
    #[must_use]
    pub fn new(
        registry: Arc<WorkflowNodeCapabilityRegistry>,
        catalog: Arc<GenerationProfileCatalog>,
        availability_reader: Arc<dyn GenerationProfileAvailabilityReaderInterface>,
    ) -> Self {
        Self { registry, catalog, availability_reader }
    }

    /// Returns only Active compatible definitions with exact matching observations.
    pub async fn list_generation_profiles_for_capability(
        &self,
        query: GenerationProfileListForCapabilityQuery,
    ) -> Result<Vec<GenerationProfileForCapabilityListItem>, GenerationProfileError> {
        self.registry
            .resolve_node_capability(query.capability_ref())
            .map_err(|_| GenerationProfileError::CapabilityNotFound)?;
        let definitions =
            self.catalog.list_active_generation_profiles_for_capability(query.capability_ref());
        if definitions.is_empty() {
            return Ok(Vec::new());
        }
        let profile_refs = definitions
            .iter()
            .map(|definition| definition.profile_ref().clone())
            .collect::<Vec<_>>();
        let request = GenerationProfileAvailabilityRequest::try_new(
            query.capability_ref().clone(),
            profile_refs,
            query.deadline(),
        )?;
        let observations =
            self.availability_reader.read_generation_profile_availability(request).await?;
        if observations.len() != definitions.len()
            || observations.iter().zip(&definitions).any(|(observation, definition)| {
                observation.profile_ref() != definition.profile_ref()
            })
        {
            return Err(GenerationProfileError::InvalidAvailabilityObservation);
        }
        Ok(definitions
            .into_iter()
            .zip(observations)
            .map(|(definition, availability)| GenerationProfileForCapabilityListItem {
                definition: definition.clone(),
                availability,
            })
            .collect())
    }
}
