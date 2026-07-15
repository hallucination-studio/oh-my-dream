use std::io::Cursor;
use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetApplicationError, AssetNodeOutputSourceLease, AssetRecordNodeOutputCommand,
};
use assets::asset::domain::{
    AssetDisplayName, AssetId, AssetMediaKind, AssetNodeOutputKey, AssetNodeOutputProduction,
    AssetOriginNodeCapabilityContractRef, AssetOriginNodeOutputKey, AssetOriginSourceAssetId,
    AssetOriginSourceAssetIds, AssetOriginWorkflowId, AssetOriginWorkflowNodeExecutionId,
    AssetOriginWorkflowNodeId, AssetOriginWorkflowRevision, AssetOriginWorkflowRunId,
    AssetWorkflowNodeOrigin,
};
use projects::project::domain::ProjectId;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[tokio::test]
async fn node_output_source_lease_exposes_one_stream_before_deadline() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let lease = AssetNodeOutputSourceLease::new(deadline, Box::pin(Cursor::new(vec![1, 2, 3])));

    assert_eq!(lease.deadline(), deadline);
    let mut stream = lease.try_take_stream().unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn node_output_source_lease_rejects_expired_handoff() {
    let lease = AssetNodeOutputSourceLease::new(
        Instant::now() - Duration::from_secs(1),
        Box::pin(Cursor::new(vec![1])),
    );
    assert!(matches!(lease.try_take_stream(), Err(AssetApplicationError::DeadlineExceeded)));
}

#[test]
fn record_node_output_command_requires_matching_producer_coordinates() {
    let producer = producer(3, 4);
    let production = production();
    let result = AssetRecordNodeOutputCommand::try_new(
        project_id(),
        AssetMediaKind::Image,
        AssetDisplayName::try_new("output").unwrap(),
        producer.clone(),
        production.clone(),
        output_key(&producer, 9),
        source_lease(),
    );
    assert!(matches!(result, Err(AssetApplicationError::IdentityConflict)));

    let command = AssetRecordNodeOutputCommand::try_new(
        project_id(),
        AssetMediaKind::Image,
        AssetDisplayName::try_new("output").unwrap(),
        producer.clone(),
        production.clone(),
        output_key(&producer, 3),
        source_lease(),
    )
    .unwrap();
    assert_eq!(command.project_id(), project_id());
    assert_eq!(command.expected_media_kind(), AssetMediaKind::Image);
    assert_eq!(command.display_name().as_str(), "output");
    assert_eq!(command.producer(), &producer);
    assert_eq!(command.production(), &production);
    assert_eq!(command.output_key().workflow_run_id(), producer.workflow_run_id());
}

fn source_lease() -> AssetNodeOutputSourceLease {
    AssetNodeOutputSourceLease::new(
        Instant::now() + Duration::from_secs(60),
        Box::pin(Cursor::new(vec![1])),
    )
}

fn producer(run_seed: u8, execution_seed: u8) -> AssetWorkflowNodeOrigin {
    AssetWorkflowNodeOrigin::new(
        AssetOriginWorkflowId::from_uuid(uuid(1)).unwrap(),
        AssetOriginWorkflowRevision::new(1).unwrap(),
        AssetOriginWorkflowRunId::from_uuid(uuid(run_seed)).unwrap(),
        AssetOriginWorkflowNodeId::from_uuid(uuid(2)).unwrap(),
        AssetOriginWorkflowNodeExecutionId::from_uuid(uuid(execution_seed)).unwrap(),
        AssetOriginNodeCapabilityContractRef::try_new("image.generate", 1, 0).unwrap(),
    )
}

fn output_key(producer: &AssetWorkflowNodeOrigin, run_seed: u8) -> AssetNodeOutputKey {
    AssetNodeOutputKey::new(
        AssetOriginWorkflowRunId::from_uuid(uuid(run_seed)).unwrap(),
        producer.node_execution_id(),
        AssetOriginNodeOutputKey::try_new("image").unwrap(),
        0,
    )
}

fn production() -> AssetNodeOutputProduction {
    AssetNodeOutputProduction::DeterministicDerived {
        source_asset_ids: AssetOriginSourceAssetIds::try_new(vec![
            AssetOriginSourceAssetId::from_asset_id(AssetId::from_uuid(uuid(8)).unwrap()),
        ])
        .unwrap(),
    }
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(7)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
