use std::time::{Duration, Instant};

use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[tokio::test]
async fn starter_fake_preserves_idempotency_conflict_and_interruption_precedence() {
    let starter = NodeCapabilityGenerationTaskStarterFakeImpl::default();
    let request =
        start_request(1, image_profile(), NodeCapabilityExecutionCancellation::active(), future());
    let first = starter.start_generation_task(request.clone()).await.unwrap();
    let replay = starter.start_generation_task(request).await.unwrap();
    assert_eq!(first, replay);

    let conflict = starter
        .start_generation_task(start_request(
            1,
            video_profile(),
            NodeCapabilityExecutionCancellation::active(),
            future(),
        ))
        .await;
    assert_eq!(conflict, Err(NodeCapabilityGenerationTaskStartFailure::Conflict));

    let cancellation = NodeCapabilityExecutionCancellation::active();
    cancellation.cancel();
    assert_eq!(
        starter
            .start_generation_task(start_request(2, image_profile(), cancellation, Instant::now()))
            .await,
        Err(NodeCapabilityGenerationTaskStartFailure::Cancelled)
    );
    assert_eq!(
        starter
            .start_generation_task(start_request(
                3,
                image_profile(),
                NodeCapabilityExecutionCancellation::active(),
                Instant::now(),
            ))
            .await,
        Err(NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded)
    );
    assert_eq!(starter.requests().len(), 3);
}

fn start_request(
    seed: u8,
    profile_ref: GenerationProfileRef,
    cancellation: NodeCapabilityExecutionCancellation,
    deadline: Instant,
) -> NodeCapabilityGenerationTaskStartRequest {
    NodeCapabilityGenerationTaskStartRequest::try_new(
        WorkflowNodeExecutionContext {
            project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(deadline),
            cancellation,
        },
        WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
            WorkflowRevision::new(1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
            contract_ref(),
        ),
        profile_ref,
        NodeCapabilityGenerationTaskRequest::Image {
            prompt: WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".into())]).unwrap(),
            aspect_ratio: ImageAspectRatio::Square,
        },
        NodeCapabilityOutputKey::new("image").unwrap(),
        Vec::new(),
    )
    .unwrap()
}

fn image_profile() -> GenerationProfileRef {
    profile("image.high_quality_general")
}
fn video_profile() -> GenerationProfileRef {
    profile("video.cinematic_image_animation")
}
fn profile(id: &str) -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new(id).unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}
fn contract_ref() -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn future() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
