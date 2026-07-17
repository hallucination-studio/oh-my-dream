use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backends::mock_generation_provider::MockGenerationProviderRegistryImpl;
use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use tasks::generation_task::*;
use uuid::Uuid;

use super::DesktopNodeCapabilityGenerationTaskStartAdapterImpl;
use crate::generation_task_storage_adapter::SqliteGenerationTaskRepositoryAdapterImpl;

#[tokio::test]
async fn production_bridge_persists_exact_image_video_and_voice_requests_before_returning() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let repository =
        SqliteGenerationTaskRepositoryAdapterImpl::try_new(connection.clone()).unwrap();
    let registry = Arc::new(MockGenerationProviderRegistryImpl::try_new().unwrap());
    let adapter = DesktopNodeCapabilityGenerationTaskStartAdapterImpl::new(
        repository.clone(),
        registry,
        GenerationTaskClockFakeImpl::new(
            GenerationTaskTimestamp::from_utc_milliseconds(100).unwrap(),
        ),
    );

    let image_start = start_request(
        1,
        "image.generate_from_text",
        "image",
        image_profile(),
        image_request("draw"),
    );
    let first = adapter.start_generation_task(image_start.clone()).await.unwrap();
    let replay = adapter.start_generation_task(image_start).await.unwrap();
    assert_eq!(first, replay);
    let image = repository
        .load_generation_task(GenerationTaskId::from_uuid(first.task_id().as_uuid()).unwrap())
        .await
        .unwrap()
        .unwrap();
    let conflict = adapter
        .start_generation_task(start_request(
            1,
            "image.generate_from_text",
            "image",
            image_profile(),
            image_request("different"),
        ))
        .await;
    assert_eq!(conflict, Err(NodeCapabilityGenerationTaskStartFailure::Conflict));
    assert_eq!(image.origin().workflow_revision(), WorkflowRevision::new(2).unwrap());
    assert_eq!(image.target().generation_profile_ref(), &image_profile());
    assert!(matches!(image.request(), GenerationTaskRequest::Image(spec)
        if spec.prompt().as_str() == "draw" && spec.aspect_ratio() == tasks::generation_task::ImageAspectRatio::Landscape16To9));

    let image_ref = managed_image(40);
    let video = start_and_load(
        &adapter,
        &repository,
        start_request(
            2,
            "video.generate_from_image",
            "video",
            video_profile(),
            video_request(image_ref),
        ),
    )
    .await;
    assert_eq!(video.target().generation_profile_ref(), &video_profile());
    assert!(matches!(video.request(), GenerationTaskRequest::Video(spec)
        if spec.input_image().asset_id().as_uuid().as_bytes() == &image_ref.asset_id().as_bytes()
            && spec.input_image().content_hash().as_bytes() == image_ref.content_fingerprint().as_bytes()
            && spec.duration_seconds() == VideoDurationSeconds::Ten
            && spec.prompt().is_some_and(|value| value.as_str() == "camera")));

    let voice = start_and_load(
        &adapter,
        &repository,
        start_request(
            3,
            "audio.synthesize_speech_from_text",
            "audio",
            voice_profile(),
            voice_request(),
        ),
    )
    .await;
    assert_eq!(voice.target().generation_profile_ref(), &voice_profile());
    assert!(matches!(voice.request(), GenerationTaskRequest::Voice(spec)
        if spec.text().as_str() == "speak"));

    let locked = connection.lock().unwrap();
    let ready: u32 = locked
        .query_row(
            "SELECT COUNT(*) FROM generation_task_outbox WHERE state='Ready' AND kind='SubmitTask'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(ready, 3);
}

async fn start_and_load<R, C>(
    adapter: &DesktopNodeCapabilityGenerationTaskStartAdapterImpl<R, C>,
    repository: &SqliteGenerationTaskRepositoryAdapterImpl,
    request: NodeCapabilityGenerationTaskStartRequest,
) -> GenerationTaskAggregate
where
    R: GenerationTaskRepositoryInterface,
    C: GenerationTaskClockInterface,
{
    let result = adapter.start_generation_task(request).await.unwrap();
    let id = GenerationTaskId::from_uuid(result.task_id().as_uuid()).unwrap();
    repository.load_generation_task(id).await.unwrap().unwrap()
}

fn start_request(
    seed: u8,
    contract_id: &str,
    output_key: &str,
    profile_ref: GenerationProfileRef,
    request: NodeCapabilityGenerationTaskRequest,
) -> NodeCapabilityGenerationTaskStartRequest {
    let input_assets = match &request {
        NodeCapabilityGenerationTaskRequest::Video { input_image, .. } => vec![*input_image],
        _ => Vec::new(),
    };
    NodeCapabilityGenerationTaskStartRequest::try_new(
        context(seed),
        WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
            WorkflowRevision::new(2).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
            contract_ref(contract_id),
        ),
        profile_ref,
        request,
        NodeCapabilityOutputKey::new(output_key).unwrap(),
        input_assets,
    )
    .unwrap()
}

fn image_request(prompt: &str) -> NodeCapabilityGenerationTaskRequest {
    NodeCapabilityGenerationTaskRequest::Image {
        prompt: text(prompt),
        aspect_ratio: nodes::ImageAspectRatio::LandscapeSixteenByNine,
    }
}

fn video_request(image: WorkflowManagedImageRef) -> NodeCapabilityGenerationTaskRequest {
    NodeCapabilityGenerationTaskRequest::Video {
        input_image: NodeCapabilityGenerationTaskAssetSnapshot::image(image),
        prompt: Some(text("camera")),
        duration_seconds: ImageToVideoDurationSeconds::Ten,
    }
}

fn voice_request() -> NodeCapabilityGenerationTaskRequest {
    NodeCapabilityGenerationTaskRequest::Voice { text: text("speak") }
}

fn text(value: &str) -> WorkflowTextValue {
    WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.into())]).unwrap()
}

fn context(seed: u8) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn managed_image(seed: u8) -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes([seed; 32]),
    )
}

fn image_profile() -> GenerationProfileRef {
    profile("image.high_quality_general")
}
fn video_profile() -> GenerationProfileRef {
    profile("video.cinematic_image_animation")
}
fn voice_profile() -> GenerationProfileRef {
    profile("speech.multilingual_narration")
}
fn profile(id: &str) -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new(id).unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}
fn contract_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
