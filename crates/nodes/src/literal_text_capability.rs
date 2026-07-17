use std::collections::BTreeMap;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::*;

/// Produces one normalized literal Text value without external dependencies.
pub struct ProvideLiteralTextCapabilityImpl {
    contract: NodeCapabilityContract,
    output_key: NodeCapabilityOutputKey,
}

impl ProvideLiteralTextCapabilityImpl {
    /// Builds the frozen `text.provide_literal@1.0` contract.
    pub fn try_new() -> Result<Self, NodeCapabilityContractError> {
        let contract_ref = NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("text.provide_literal")?,
            NodeCapabilityContractVersion::new(1, 0)?,
        );
        let output_key = NodeCapabilityOutputKey::new("text")?;
        let contract = NodeCapabilityContract::try_new(
            contract_ref,
            vec![NodeCapabilityParameterContract::required(
                NodeCapabilityParameterKey::new("text")?,
                NodeCapabilityParameterConstraint::text_utf8_bytes(1, 65_536)?,
            )],
            Vec::new(),
            vec![NodeCapabilityOutputContract::new(
                output_key.clone(),
                WorkflowDataType::Text,
                true,
            )],
            NodeCapabilityExecutionKind::PureValue,
        )?;
        Ok(Self { contract, output_key })
    }
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for ProvideLiteralTextCapabilityImpl {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }

    async fn check_node_external_readiness(
        &self,
        request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        if normalized_literal_text(&request.normalized_parameters).is_some() {
            Vec::new()
        } else {
            vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()]
        }
    }

    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        let Some(text) = normalized_literal_text(&request.normalized_parameters)
            .filter(|_| request.inputs.is_empty())
            .filter(|_| request.origin.capability_contract_ref() == self.contract.contract_ref())
        else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        if let Some(error) = cancelled_or_elapsed_error(&self.contract, &request) {
            return Err(error);
        }
        let text = WorkflowTextValue::try_new([WorkflowTextPart::Literal(text.to_owned())])
            .map_err(|_| invalid_output(&self.contract, &request, &self.output_key))?;
        let mut values = BTreeMap::new();
        values.insert(self.output_key.clone(), WorkflowRuntimeValue::Text(text));
        WorkflowNodeOutputSet::try_new(&self.contract, values)
            .map_err(|_| invalid_output(&self.contract, &request, &self.output_key))
    }
}

fn normalized_literal_text(parameters: &NodeCapabilityNormalizedParameters) -> Option<&str> {
    let key = NodeCapabilityParameterKey::new("text").ok()?;
    match parameters.get(&key)? {
        NodeCapabilityParameterValue::Text(value)
            if parameters.len() == 1 && (1..=65_536).contains(&value.len()) =>
        {
            Some(value)
        }
        _ => None,
    }
}

fn cancelled_or_elapsed_error(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> Option<NodeCapabilityExecutionError> {
    let contract_ref = contract.contract_ref().clone();
    let node_execution_id = request.context.node_execution_id;
    if request.context.cancellation.is_cancelled() {
        return Some(NodeCapabilityExecutionError::cancelled_while_resolving_inputs(
            contract_ref,
            node_execution_id,
        ));
    }
    if request.context.deadline.is_reached_at(Instant::now()) {
        return Some(NodeCapabilityExecutionError::deadline_exceeded_while_resolving_inputs(
            contract_ref,
            node_execution_id,
        ));
    }
    None
}

fn invalid_invocation(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> NodeCapabilityExecutionError {
    NodeCapabilityExecutionError::invalid_capability_invocation(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
    )
}

fn invalid_output(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
) -> NodeCapabilityExecutionError {
    NodeCapabilityExecutionError::invalid_result_while_assembling_outputs(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
        output_key.clone(),
    )
}
