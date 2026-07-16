use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use backends::provider_routing::{
    ImageToVideoProviderRouteInterface, ImageToVideoProviderRouterImpl,
};
use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    WorkflowManagedAssetIdBoundaryValue, WorkflowManagedContentFingerprint,
    WorkflowManagedImageRef, WorkflowNodeExecutionContext, WorkflowNodeExecutionId, WorkflowRunId,
};
use nodes::{
    GenerationProfileId, GenerationProfileRef, GenerationProfileVersion,
    ImageToVideoDurationSeconds, ImageToVideoProviderInterface, ImageToVideoProviderRequest,
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaContentDigest,
    NodeCapabilityMediaMimeType, NodeCapabilityMediaSourceLease, NodeCapabilityReadableImageInput,
};
use projects::project::domain::ProjectId;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::*;
use crate::{
    credential_repository::{
        GenerationProviderCredentialRepositoryError, GenerationProviderCredentialSecret,
    },
    provider_adapters::fal::{
        FalHttpResponse, FalHttpTransportInterface, FalQueueRequest, FalTransportError,
    },
};

#[tokio::test]
async fn sends_exact_data_uri_wire_and_returns_silent_mp4() {
    let video = mp4_bytes();
    let transport = Arc::new(ScriptedTransport::new(
        [
            json_response(202, json!({"request_id": "video_1"})),
            json_response(200, json!({"status": "COMPLETED"})),
            json_response(
                200,
                json!({
                    "video": {
                        "url": "https://v3b.fal.media/files/test.mp4",
                        "content_type": "video/mp4",
                        "file_size": video.len()
                    }
                }),
            ),
        ],
        FalHttpResponse {
            status: 200,
            content_type: Some("video/mp4".into()),
            body: video.clone(),
        },
    ));
    let route = FalImageToVideoProviderRouteImpl::with_transport(
        credential_id(),
        Arc::new(FakeRepository),
        transport.clone(),
    );
    let router = ImageToVideoProviderRouterImpl::try_new([(
        profile(),
        Arc::new(route) as Arc<dyn ImageToVideoProviderRouteInterface>,
    )])
    .unwrap();

    let payload = router
        .generate_video_from_image(ImageToVideoProviderRequest::new(
            profile(),
            context(),
            readable_image(vec![1, 2, 3, 4]),
            None,
            ImageToVideoDurationSeconds::Ten,
        ))
        .await
        .unwrap();
    assert_eq!(
        payload.facts(),
        NodeCapabilityDeclaredMediaFacts::try_video(64, 32, 10_000, false).unwrap()
    );
    let mut bytes = Vec::new();
    payload.into_source().try_take_stream().unwrap().read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, video);

    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].url, submit_url());
    let body: serde_json::Value =
        serde_json::from_slice(requests[0].json_body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({
            "start_image_url": "data:image/png;base64,AQIDBA==",
            "prompt": wire::DEFAULT_PROMPT,
            "duration": "10",
            "generate_audio": false
        })
    );
    assert_eq!(requests[1].url, status_url("video_1"));
    assert_eq!(requests[2].url, result_url("video_1"));
}

struct FakeRepository;

#[async_trait]
impl GenerationProviderCredentialRepositoryInterface for FakeRepository {
    async fn save_generation_provider_credential(
        &self,
        _id: GenerationProviderCredentialId,
        _secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        Ok(())
    }

    async fn load_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialRepositoryError>
    {
        GenerationProviderCredentialSecret::new(b"test-credential".to_vec())
    }

    async fn delete_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        Ok(())
    }
}

struct ScriptedTransport {
    queue: Mutex<VecDeque<FalHttpResponse>>,
    download: Mutex<Option<FalHttpResponse>>,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl ScriptedTransport {
    fn new(queue: impl IntoIterator<Item = FalHttpResponse>, download: FalHttpResponse) -> Self {
        Self {
            queue: Mutex::new(queue.into_iter().collect()),
            download: Mutex::new(Some(download)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl FalHttpTransportInterface for ScriptedTransport {
    async fn send_queue_request(
        &self,
        request: FalQueueRequest,
        secret: &GenerationProviderCredentialSecret,
    ) -> Result<FalHttpResponse, FalTransportError> {
        assert_eq!(secret.as_bytes(), b"test-credential");
        self.requests
            .lock()
            .unwrap()
            .push(RecordedRequest { url: request.url, json_body: request.json_body });
        self.queue.lock().unwrap().pop_front().ok_or(FalTransportError::NetworkBeforeSubmission)
    }

    async fn download_media(
        &self,
        url: &str,
        _deadline: Instant,
        maximum_bytes: usize,
    ) -> Result<FalHttpResponse, FalTransportError> {
        assert_eq!(url, "https://v3b.fal.media/files/test.mp4");
        assert_eq!(maximum_bytes, MAX_VIDEO_BYTES);
        self.download.lock().unwrap().take().ok_or(FalTransportError::DownloadRejected)
    }
}

#[derive(Clone)]
struct RecordedRequest {
    url: String,
    json_body: Option<Vec<u8>>,
}

fn readable_image(bytes: Vec<u8>) -> NodeCapabilityReadableImageInput {
    let digest = digest(&bytes);
    NodeCapabilityReadableImageInput::try_new(
        WorkflowManagedImageRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(id(4).into_bytes()).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
        ),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(64, 32).unwrap(),
        NodeCapabilityMediaSourceLease::try_new(
            bytes.len() as u64,
            digest,
            Instant::now() + Duration::from_secs(5),
            Box::pin(Cursor::new(bytes)),
        )
        .unwrap(),
    )
    .unwrap()
}

fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn json_response(status: u16, value: serde_json::Value) -> FalHttpResponse {
    FalHttpResponse {
        status,
        content_type: Some("application/json".into()),
        body: serde_json::to_vec(&value).unwrap(),
    }
}

fn credential_id() -> GenerationProviderCredentialId {
    GenerationProviderCredentialId::new("fal.primary").unwrap()
}

fn profile() -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new("video.cinematic_image_animation").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}

fn context() -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(id(1)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(id(2)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(id(3)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn id(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn mp4_bytes() -> Vec<u8> {
    vec![0, 0, 0, 16, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm', 0, 0, 0, 0]
}
