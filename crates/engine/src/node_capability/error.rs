//! Closed failures exposed by node-capability contracts.

use thiserror::Error;

use super::{
    NodeCapabilityContractRef, NodeCapabilityExecutionErrorConstructionError,
    NodeCapabilityExecutionFailure, NodeCapabilityExecutionStage, NodeCapabilityExecutionTarget,
    NodeCapabilityInputKey, NodeCapabilityMediaFailure, NodeCapabilityOutputKey,
    NodeCapabilityParameterKey, NodeCapabilityProviderFailure, WorkflowNodeExecutionId,
    readiness_parameter_key,
};

/// Structured safe failure returned by one capability execution.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("node capability execution failed")]
pub struct NodeCapabilityExecutionError {
    contract_ref: NodeCapabilityContractRef,
    node_execution_id: WorkflowNodeExecutionId,
    stage: NodeCapabilityExecutionStage,
    failure: Box<NodeCapabilityExecutionFailure>,
    target: NodeCapabilityExecutionTarget,
}

impl NodeCapabilityExecutionError {
    /// Creates the fixed error for a malformed direct capability invocation.
    #[must_use]
    pub fn invalid_capability_invocation(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::InvalidCapabilityInvocation,
        )
    }

    /// Creates the fixed error for invalid final output-set assembly.
    #[must_use]
    pub fn invalid_result_while_assembling_outputs(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::AssembleOutputs,
            failure: Box::new(NodeCapabilityExecutionFailure::InvalidCapabilityResult),
            target: NodeCapabilityExecutionTarget::Output(output_key),
        }
    }

    /// Creates the fixed error for invalid produced-media write-request construction.
    #[must_use]
    pub fn invalid_result_while_constructing_media_write(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ValidateProviderResult,
            failure: Box::new(NodeCapabilityExecutionFailure::InvalidCapabilityResult),
            target: NodeCapabilityExecutionTarget::Output(output_key),
        }
    }

    /// Creates a cancellation observed while resolving capability inputs.
    #[must_use]
    pub fn cancelled_while_resolving_inputs(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::Cancelled,
        )
    }

    /// Creates a deadline failure observed while resolving capability inputs.
    #[must_use]
    pub fn deadline_exceeded_while_resolving_inputs(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
        )
    }

    /// Creates cancellation observed while resolving one parameter.
    #[must_use]
    pub fn cancelled_while_resolving_parameter(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: Box::new(NodeCapabilityExecutionFailure::Cancelled),
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    /// Creates deadline failure observed while resolving one parameter.
    #[must_use]
    pub fn deadline_exceeded_while_resolving_parameter(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: Box::new(NodeCapabilityExecutionFailure::DeadlineExceeded),
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    /// Creates an exact managed-media failure for one Asset parameter resolution.
    #[must_use]
    pub fn managed_media_parameter_resolution_failed(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
        failure: NodeCapabilityMediaFailure,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: Box::new(NodeCapabilityExecutionFailure::Media(failure)),
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    /// Creates cancellation observed while resolving one runtime input.
    #[must_use]
    pub fn cancelled_while_resolving_input(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        input_key: NodeCapabilityInputKey,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::ResolveInputs,
            NodeCapabilityExecutionFailure::Cancelled,
            NodeCapabilityExecutionTarget::Input(input_key),
        )
    }

    /// Creates deadline failure observed while resolving one runtime input.
    #[must_use]
    pub fn deadline_exceeded_while_resolving_input(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        input_key: NodeCapabilityInputKey,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::ResolveInputs,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
            NodeCapabilityExecutionTarget::Input(input_key),
        )
    }

    /// Creates managed-media failure observed while resolving one runtime input.
    #[must_use]
    pub fn managed_media_input_resolution_failed(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        input_key: NodeCapabilityInputKey,
        failure: NodeCapabilityMediaFailure,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::ResolveInputs,
            NodeCapabilityExecutionFailure::Media(failure),
            NodeCapabilityExecutionTarget::Input(input_key),
        )
    }

    /// Creates an exact provider call failure.
    #[must_use]
    pub fn provider_call_failed(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        failure: NodeCapabilityProviderFailure,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::CallProvider,
            NodeCapabilityExecutionFailure::Provider(failure),
            NodeCapabilityExecutionTarget::Capability,
        )
    }

    /// Creates cancellation observed while calling the provider.
    #[must_use]
    pub fn cancelled_while_calling_provider(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::CallProvider,
            NodeCapabilityExecutionFailure::Cancelled,
            NodeCapabilityExecutionTarget::Capability,
        )
    }

    /// Creates deadline failure observed while calling the provider.
    #[must_use]
    pub fn deadline_exceeded_while_calling_provider(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::CallProvider,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
            NodeCapabilityExecutionTarget::Capability,
        )
    }

    /// Creates exact managed-media failure observed while writing one output.
    #[must_use]
    pub fn managed_media_output_write_failed(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
        failure: NodeCapabilityMediaFailure,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionFailure::Media(failure),
            NodeCapabilityExecutionTarget::Output(output_key),
        )
    }

    /// Creates cancellation observed while writing one output.
    #[must_use]
    pub fn cancelled_while_writing_output(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionFailure::Cancelled,
            NodeCapabilityExecutionTarget::Output(output_key),
        )
    }

    /// Creates deadline failure observed while writing one output.
    #[must_use]
    pub fn deadline_exceeded_while_writing_output(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
    ) -> Self {
        Self::operation_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
            NodeCapabilityExecutionTarget::Output(output_key),
        )
    }

    fn operation_failure(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        stage: NodeCapabilityExecutionStage,
        failure: NodeCapabilityExecutionFailure,
        target: NodeCapabilityExecutionTarget,
    ) -> Self {
        Self { contract_ref, node_execution_id, stage, failure: Box::new(failure), target }
    }

    fn resolve_inputs_capability_failure(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        failure: NodeCapabilityExecutionFailure,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: Box::new(failure),
            target: NodeCapabilityExecutionTarget::Capability,
        }
    }
    /// Creates an error only when its stage and target agree.
    pub fn try_new(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        stage: NodeCapabilityExecutionStage,
        failure: NodeCapabilityExecutionFailure,
        target: NodeCapabilityExecutionTarget,
    ) -> Result<Self, NodeCapabilityExecutionErrorConstructionError> {
        let valid_target = match stage {
            NodeCapabilityExecutionStage::ResolveInputs => matches!(
                target,
                NodeCapabilityExecutionTarget::Capability
                    | NodeCapabilityExecutionTarget::Parameter(_)
                    | NodeCapabilityExecutionTarget::Input(_)
            ),
            NodeCapabilityExecutionStage::CallProvider => {
                matches!(target, NodeCapabilityExecutionTarget::Capability)
            }
            NodeCapabilityExecutionStage::ValidateProviderResult
            | NodeCapabilityExecutionStage::WriteManagedMedia => matches!(
                target,
                NodeCapabilityExecutionTarget::Capability
                    | NodeCapabilityExecutionTarget::Output(_)
            ),
            NodeCapabilityExecutionStage::AssembleOutputs => {
                matches!(target, NodeCapabilityExecutionTarget::Output(_))
            }
        };
        let readiness_target_matches = match (&failure, &target) {
            (
                NodeCapabilityExecutionFailure::Readiness(issue),
                NodeCapabilityExecutionTarget::Parameter(execution_parameter_key),
            ) => readiness_parameter_key(issue).is_some_and(|key| key == execution_parameter_key),
            (NodeCapabilityExecutionFailure::Readiness(_), _) => false,
            _ => true,
        };
        let invalid_invocation_shape = failure
            == NodeCapabilityExecutionFailure::InvalidCapabilityInvocation
            && (stage != NodeCapabilityExecutionStage::ResolveInputs
                || target != NodeCapabilityExecutionTarget::Capability);
        let invalid_result_shape = failure
            == NodeCapabilityExecutionFailure::InvalidCapabilityResult
            && !matches!(
                (&stage, &target),
                (
                    NodeCapabilityExecutionStage::ValidateProviderResult
                        | NodeCapabilityExecutionStage::AssembleOutputs,
                    NodeCapabilityExecutionTarget::Output(_)
                )
            );
        let assemble_outputs_failure_shape = stage == NodeCapabilityExecutionStage::AssembleOutputs
            && failure != NodeCapabilityExecutionFailure::InvalidCapabilityResult;
        if !valid_target
            || !readiness_target_matches
            || invalid_invocation_shape
            || invalid_result_shape
            || assemble_outputs_failure_shape
        {
            return Err(NodeCapabilityExecutionErrorConstructionError);
        }
        Ok(Self { contract_ref, node_execution_id, stage, failure: Box::new(failure), target })
    }

    /// Returns the exact capability contract that failed.
    #[must_use]
    pub const fn contract_ref(&self) -> &NodeCapabilityContractRef {
        &self.contract_ref
    }
    /// Returns the planned node execution that failed.
    #[must_use]
    pub const fn node_execution_id(&self) -> WorkflowNodeExecutionId {
        self.node_execution_id
    }
    /// Returns the exact failure stage.
    #[must_use]
    pub const fn stage(&self) -> NodeCapabilityExecutionStage {
        self.stage
    }
    /// Returns the closed failure source.
    #[must_use]
    pub const fn failure(&self) -> &NodeCapabilityExecutionFailure {
        &self.failure
    }
    /// Returns the safe structured failure target.
    #[must_use]
    pub const fn target(&self) -> &NodeCapabilityExecutionTarget {
        &self.target
    }
}
