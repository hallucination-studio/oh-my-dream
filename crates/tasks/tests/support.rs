use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use engine::node_capability::{WorkflowNodeExecutionId, WorkflowRunId};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use projects::project::domain::ProjectId;
use tasks::generation_task::domain::*;
use uuid::Uuid;

pub fn uuid(seed: u128) -> Uuid {
    let mut bytes = seed.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub fn time(value: i64) -> GenerationTaskTimestamp {
    GenerationTaskTimestamp::from_utc_milliseconds(value).unwrap()
}

pub fn origin(seed: u128) -> GenerationTaskOrigin {
    GenerationTaskOrigin::new(
        ProjectId::from_uuid(uuid(seed)).unwrap(),
        WorkflowId::from_uuid(uuid(seed + 1)).unwrap(),
        WorkflowRunId::from_uuid(uuid(seed + 2)).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed + 3)).unwrap(),
        WorkflowNodeExecutionId::from_uuid(uuid(seed + 4)).unwrap(),
    )
}

pub fn target(route: &str) -> GenerationTaskTarget {
    GenerationTaskTarget::new(
        GenerationProfileRef::new(
            GenerationProfileId::try_new("image.high_quality_general").unwrap(),
            GenerationProfileVersion::try_new(1).unwrap(),
        ),
        GenerationProviderId::try_new("mock").unwrap(),
        GenerationProviderRouteId::try_new(route).unwrap(),
    )
}

pub fn image_request(prompt: &str) -> GenerationTaskRequest {
    GenerationTaskRequest::Image(ImageGenerationSpec::new(
        GenerationTaskText::try_new(prompt).unwrap(),
        ImageAspectRatio::Square,
    ))
}

pub fn text_request() -> GenerationTaskRequest {
    GenerationTaskRequest::Text(TextGenerationSpec::new(
        GenerationTaskText::try_new("text prompt").unwrap(),
    ))
}

pub fn voice_request() -> GenerationTaskRequest {
    GenerationTaskRequest::Voice(VoiceGenerationSpec::new(
        GenerationTaskText::try_new("speech text").unwrap(),
    ))
}

pub fn video_request() -> GenerationTaskRequest {
    GenerationTaskRequest::Video(
        VideoGenerationSpec::try_new(
            AssetSnapshotRef::new(
                AssetId::from_uuid(uuid(60)).unwrap(),
                AssetMediaKind::Image,
                AssetContentDigest::from_bytes([6; 32]),
            ),
            VideoDurationSeconds::Five,
            Some(GenerationTaskText::try_new("animate gently").unwrap()),
        )
        .unwrap(),
    )
}

pub fn result_for(request: &GenerationTaskRequest) -> GenerationTaskResult {
    match request.kind() {
        GenerationTaskRequestKind::Text => GenerationTaskResult::Text {
            content: GenerationTaskText::try_new("generated text").unwrap(),
        },
        GenerationTaskRequestKind::Image => asset_result(AssetMediaKind::Image),
        GenerationTaskRequestKind::Voice => asset_result(AssetMediaKind::Audio),
        GenerationTaskRequestKind::Video => asset_result(AssetMediaKind::Video),
    }
}

pub fn asset_result(media_kind: AssetMediaKind) -> GenerationTaskResult {
    let seed = match media_kind {
        AssetMediaKind::Image => 71,
        AssetMediaKind::Video => 72,
        AssetMediaKind::Audio => 73,
    };
    GenerationTaskResult::Asset(GenerationTaskAssetResult::new(
        AssetId::from_uuid(uuid(seed)).unwrap(),
        media_kind,
    ))
}

pub fn failure() -> GenerationTaskFailure {
    GenerationTaskFailure::try_new(
        GenerationTaskFailureKind::ProviderRejected,
        "PROVIDER_REJECTED",
        "The provider rejected generation.",
    )
    .unwrap()
}

pub fn task_with_request(request: GenerationTaskRequest) -> GenerationTaskAggregate {
    GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(80)).unwrap(),
        origin(1),
        GenerationTaskIdempotencyKey::try_new("node-execution-1").unwrap(),
        target("mock.image.high-quality-general.v1"),
        request,
        time(100),
        time(30_100),
    )
    .unwrap()
}

pub fn new_task() -> GenerationTaskAggregate {
    task_with_request(image_request("a quiet mountain lake"))
}

pub fn handle() -> GenerationProviderTaskHandle {
    GenerationProviderTaskHandle::try_new("remote-1").unwrap()
}

pub fn restore_state(
    state: GenerationTaskState,
    result: Option<GenerationTaskResult>,
) -> Result<GenerationTaskAggregate, GenerationTaskDomainError> {
    let task = new_task();
    GenerationTaskAggregate::restore(
        task.id(),
        task.origin().clone(),
        task.idempotency_key().clone(),
        task.request_hash(),
        task.target().clone(),
        task.request().clone(),
        task.provider_deadline_at(),
        state,
        result,
        task.created_at(),
        time(120),
        GenerationTaskRevision::try_new(2).unwrap(),
    )
}
