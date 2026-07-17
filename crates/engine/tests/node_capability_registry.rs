use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
    NodeCapabilityExecutionRequest, NodeCapabilityNormalizedParameters,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterError,
    NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
    NodeCapabilityRegistryError, WorkflowDataType, WorkflowNodeCapabilityInterface,
    WorkflowNodeCapabilityRegistry, WorkflowNodeOutputSet,
};

#[test]
fn immutable_registry_lists_refs_in_order_and_rejects_duplicates() {
    let later: Arc<dyn WorkflowNodeCapabilityInterface> =
        Arc::new(FakeCapabilityImpl::new("video.generate_from_image", WorkflowDataType::Video));
    let earlier: Arc<dyn WorkflowNodeCapabilityInterface> =
        Arc::new(FakeCapabilityImpl::new("image.generate_from_text", WorkflowDataType::Image));
    let registry = WorkflowNodeCapabilityRegistry::try_new([later, Arc::clone(&earlier)]).unwrap();

    let listed = registry.list_node_capability_contracts();
    assert_eq!(listed[0].contract_ref().to_string(), "image.generate_from_text@1.0");
    assert_eq!(listed[1].contract_ref().to_string(), "video.generate_from_image@1.0");
    assert!(Arc::ptr_eq(
        &registry
            .resolve_node_capability(earlier.node_capability_contract().contract_ref())
            .unwrap(),
        &earlier
    ));
    assert!(matches!(
        WorkflowNodeCapabilityRegistry::try_new([Arc::clone(&earlier), Arc::clone(&earlier)]),
        Err(NodeCapabilityRegistryError::DuplicateContractRef { .. })
    ));
}

struct FakeCapabilityImpl {
    contract: NodeCapabilityContract,
}

impl FakeCapabilityImpl {
    fn new(id: &str, output_type: WorkflowDataType) -> Self {
        Self {
            contract: NodeCapabilityContract::try_new(
                capability_ref(id),
                Vec::new(),
                Vec::new(),
                vec![NodeCapabilityOutputContract::new(
                    NodeCapabilityOutputKey::new("output").unwrap(),
                    output_type,
                    true,
                )],
                NodeCapabilityExecutionKind::ContentGeneration,
            )
            .unwrap(),
        }
    }
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for FakeCapabilityImpl {
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
        _request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        Vec::new()
    }

    async fn execute_node_capability(
        &self,
        _request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        unreachable!("registry tests do not execute capabilities")
    }
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
