//! Consumer-owned substitution boundary for exact node capabilities.

use async_trait::async_trait;

use super::{
    NodeCapabilityContract, NodeCapabilityExecutionError, NodeCapabilityExecutionRequest,
    NodeCapabilityNormalizedParameters, NodeCapabilityParameterError, NodeCapabilityParameterSet,
    NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
    WorkflowNodeCapabilityExecutionOutcome,
};

/// Workflow-owned behavior contract implemented by every exact node capability.
#[async_trait]
pub trait WorkflowNodeCapabilityInterface: Send + Sync {
    /// Returns the exact immutable structural contract.
    fn node_capability_contract(&self) -> &NodeCapabilityContract;

    /// Validates supplied parameters and inserts only declared defaults.
    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError>;

    /// Checks parameter-selected external state without dispatching work.
    async fn check_node_external_readiness(
        &self,
        request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue>;

    /// Executes one admitted node against exact normalized inputs.
    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError>;
}
