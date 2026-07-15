use std::time::{Duration, Instant};

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityExecutionError, NodeCapabilityExecutionFailure, NodeCapabilityExecutionStage,
    NodeCapabilityExecutionTarget, NodeCapabilityOutputKey, NodeCapabilityProviderFailure,
    NodeCapabilityProviderFailureCategory, NodeCapabilityReadinessCategory,
    NodeCapabilityReadinessDeadline, NodeCapabilityReadinessIssue, NodeCapabilityReadinessTarget,
    WorkflowManagedAssetIdBoundaryValue, WorkflowNodeExecutionId,
};
use uuid::Uuid;

#[test]
fn cancellation_is_initially_active_and_idempotently_shared_between_clones() {
    let cancellation = NodeCapabilityExecutionCancellation::active();
    let clone = cancellation.clone();
    assert!(!clone.is_cancelled());
    cancellation.cancel();
    cancellation.cancel();
    assert!(clone.is_cancelled());
}

#[test]
fn execution_deadline_uses_only_supplied_monotonic_observations() {
    let now = Instant::now();
    let deadline = NodeCapabilityExecutionDeadline::at(now + Duration::from_secs(1));
    assert!(!deadline.is_reached_at(now));
    assert!(deadline.is_reached_at(now + Duration::from_secs(1)));
    assert_eq!(deadline.monotonic_instant(), now + Duration::from_secs(1));
}

#[test]
fn readiness_deadline_exposes_the_same_boundary_instant() {
    let instant = Instant::now() + Duration::from_secs(2);
    let deadline = NodeCapabilityReadinessDeadline::at(instant);
    assert_eq!(deadline.monotonic_instant(), instant);
    assert!(deadline.is_reached_at(instant));
}

#[test]
fn readiness_issue_rejects_category_target_mismatch() {
    let result = NodeCapabilityReadinessIssue::try_new(
        NodeCapabilityReadinessCategory::GenerationProfileUnavailable,
        NodeCapabilityReadinessTarget::ManagedAsset {
            parameter_key: engine::node_capability::NodeCapabilityParameterKey::new("asset_id")
                .unwrap(),
            asset_id: WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(9)).unwrap(),
        },
        None,
    );
    assert!(result.is_err());
}

#[test]
fn readiness_issue_rejects_equal_kinds_for_a_kind_mismatch() {
    let result = NodeCapabilityReadinessIssue::try_new(
        NodeCapabilityReadinessCategory::ManagedAssetKindMismatch,
        NodeCapabilityReadinessTarget::ManagedAsset {
            parameter_key: engine::node_capability::NodeCapabilityParameterKey::new("asset_id")
                .unwrap(),
            asset_id: WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(8)).unwrap(),
        },
        Some((
            engine::node_capability::WorkflowDataType::Image,
            engine::node_capability::WorkflowDataType::Image,
        )),
    );
    assert!(result.is_err());
}

#[test]
fn execution_error_rejects_output_target_during_provider_call() {
    let result = NodeCapabilityExecutionError::try_new(
        capability_ref("image.generate_from_text"),
        WorkflowNodeExecutionId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(3))).unwrap(),
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionFailure::Cancelled,
        NodeCapabilityExecutionTarget::Output(NodeCapabilityOutputKey::new("image").unwrap()),
    );
    assert!(result.is_err());
}

#[test]
fn execution_error_rejects_a_different_parameter_than_its_readiness_issue() {
    let issue = NodeCapabilityReadinessIssue::try_new(
        NodeCapabilityReadinessCategory::ManagedAssetUnavailable,
        NodeCapabilityReadinessTarget::ManagedAsset {
            parameter_key: engine::node_capability::NodeCapabilityParameterKey::new("asset_id")
                .unwrap(),
            asset_id: WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(4)).unwrap(),
        },
        None,
    )
    .unwrap();
    let result = NodeCapabilityExecutionError::try_new(
        capability_ref("image.read_managed_asset"),
        WorkflowNodeExecutionId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(5))).unwrap(),
        NodeCapabilityExecutionStage::ResolveInputs,
        NodeCapabilityExecutionFailure::Readiness(issue),
        NodeCapabilityExecutionTarget::Parameter(
            engine::node_capability::NodeCapabilityParameterKey::new("other_asset_id").unwrap(),
        ),
    );

    assert!(result.is_err());
}

#[test]
fn provider_failure_rejects_retry_time_for_non_retryable_category() {
    let observed_at = Instant::now();
    let result = NodeCapabilityProviderFailure::try_new(
        NodeCapabilityProviderFailureCategory::AuthenticationFailed,
        false,
        observed_at,
        Some(observed_at + Duration::from_secs(1)),
    );
    assert!(result.is_err());
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn uuid_v4_bytes(seed: u8) -> [u8; 16] {
    [seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed]
}
