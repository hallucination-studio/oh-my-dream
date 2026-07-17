use std::time::Instant;

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityExecutionError, NodeCapabilityGenerationTaskStartFailure,
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory, WorkflowNodeExecutionId,
};
use engine::workflow::{WorkflowGenerationTaskFailure, WorkflowNodeExecutionFailure};
use uuid::Uuid;

use super::{decode_execution_failure, encode_execution_failure};

#[test]
fn round_trips_structured_provider_execution_failure() {
    let contract_ref = NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("test.provider").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    );
    let execution_id = WorkflowNodeExecutionId::from_uuid(
        Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap(),
    )
    .unwrap();
    let provider_failure = NodeCapabilityProviderFailure::try_new(
        NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        false,
        Instant::now(),
        None,
    )
    .unwrap();
    let failure = WorkflowNodeExecutionFailure::Capability(
        NodeCapabilityExecutionError::provider_call_failed(
            contract_ref,
            execution_id,
            provider_failure,
        ),
    );

    let restored = decode_execution_failure(encode_execution_failure(&failure)).unwrap();

    assert_eq!(restored, failure);
}

#[test]
fn round_trips_generation_task_start_execution_failure() {
    let contract_ref = NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    );
    let execution_id = WorkflowNodeExecutionId::from_uuid(
        Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap(),
    )
    .unwrap();
    let failure = WorkflowNodeExecutionFailure::Capability(
        NodeCapabilityExecutionError::generation_task_start_failed(
            contract_ref,
            execution_id,
            NodeCapabilityGenerationTaskStartFailure::Persistence,
        ),
    );

    let restored = decode_execution_failure(encode_execution_failure(&failure)).unwrap();

    assert_eq!(restored, failure);
}

#[test]
fn round_trips_terminal_generation_task_failure() {
    let failure = WorkflowNodeExecutionFailure::GenerationTask(
        WorkflowGenerationTaskFailure::GenerationTaskCancelled,
    );

    let restored = decode_execution_failure(encode_execution_failure(&failure)).unwrap();

    assert_eq!(restored, failure);
}
