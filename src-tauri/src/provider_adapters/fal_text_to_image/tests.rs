use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use backends::provider_routing::{
    TextToImageProviderRouteInterface, TextToImageProviderRouterImpl,
};
use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityProviderFailureCategory, WorkflowNodeExecutionContext, WorkflowNodeExecutionId,
    WorkflowRunId, WorkflowTextPart, WorkflowTextValue,
};
use nodes::{
    GenerationProfileAvailabilityState, GenerationProfileId, GenerationProfileRef,
    GenerationProfileUnavailableReason, GenerationProfileVersion, ImageAspectRatio,
    TextToImageProviderInterface, TextToImageProviderRequest,
};
use projects::project::domain::ProjectId;
use serde_json::json;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::*;
use crate::{
    credential_vault::{
        GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError,
    },
    provider_adapters::fal::{
        FalHttpResponse, FalHttpTransportInterface, FalQueueRequest, FalTransportError,
    },
};

#[tokio::test]
async fn sends_exact_frozen_queue_wire_and_returns_downloaded_png() {
    let png = png_bytes(64, 32);
    let transport = Arc::new(ScriptedTransport::new(
        [
            json_response(202, json!({"request_id": "request_1"})),
            json_response(200, json!({"status": "IN_QUEUE"})),
            json_response(200, json!({"status": "COMPLETED"})),
            json_response(
                200,
                json!({
                    "images": [{
                        "url": "https://v3b.fal.media/files/test.png",
                        "content_type": "image/png",
                        "width": 64,
                        "height": 32
                    }],
                    "has_nsfw_concepts": [false]
                }),
            ),
        ],
        FalHttpResponse { status: 200, content_type: Some("image/png".into()), body: png.clone() },
    ));
    let route = FalTextToImageProviderRouteImpl::with_transport(
        credential_id(),
        Arc::new(FakeVault),
        transport.clone(),
    );
    let router = TextToImageProviderRouterImpl::try_new([(
        profile(),
        Arc::new(route) as Arc<dyn TextToImageProviderRouteInterface>,
    )])
    .unwrap();

    let payload = router
        .generate_image_from_text(TextToImageProviderRequest::new(
            profile(),
            context(),
            WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw a moon".into())]).unwrap(),
            ImageAspectRatio::LandscapeFourByThree,
        ))
        .await
        .unwrap();
    let mut stream = payload.into_source().try_take_stream().unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();

    assert_eq!(bytes, png);
    let requests = transport.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].url, format!("{QUEUE_ROOT}/{}", NATIVE_MODEL_ID.as_str()));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(requests[0].json_body.as_deref().unwrap())
            .unwrap(),
        json!({
            "prompt": "draw a moon",
            "aspect_ratio": "4:3",
            "num_images": 1,
            "output_format": "png",
            "safety_tolerance": "2",
            "enhance_prompt": false
        })
    );
    assert!(requests[1].url.ends_with("/requests/request_1/status"));
    assert!(requests[2].url.ends_with("/requests/request_1/status"));
    assert!(requests[3].url.ends_with("/requests/request_1"));
}

#[tokio::test]
async fn reports_missing_credential_without_exposing_or_calling_transport() {
    let transport = Arc::new(ScriptedTransport::new(
        [],
        FalHttpResponse { status: 500, content_type: None, body: vec![] },
    ));
    let route = FalTextToImageProviderRouteImpl::with_transport(
        credential_id(),
        Arc::new(MissingVault),
        transport.clone(),
    );

    let availability = route.observe_provider_route_availability().await;

    assert_eq!(
        availability,
        GenerationProfileAvailabilityState::Unavailable {
            reason: GenerationProfileUnavailableReason::AuthenticationRequired,
            retry_after: None,
        }
    );
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn rejects_unknown_queue_status_without_fetching_a_result() {
    let transport = Arc::new(ScriptedTransport::new(
        [
            json_response(202, json!({"request_id": "request_2"})),
            json_response(200, json!({"status": "UNKNOWN"})),
        ],
        FalHttpResponse {
            status: 200,
            content_type: Some("image/png".into()),
            body: png_bytes(1, 1),
        },
    ));
    let route = FalTextToImageProviderRouteImpl::with_transport(
        credential_id(),
        Arc::new(FakeVault),
        transport.clone(),
    );
    let router = TextToImageProviderRouterImpl::try_new([(
        profile(),
        Arc::new(route) as Arc<dyn TextToImageProviderRouteInterface>,
    )])
    .unwrap();

    let result = router
        .generate_image_from_text(TextToImageProviderRequest::new(
            profile(),
            context(),
            WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".into())]).unwrap(),
            ImageAspectRatio::Square,
        ))
        .await;
    let Err(error) = result else { panic!("unknown queue status was accepted") };

    assert_eq!(error.category(), NodeCapabilityProviderFailureCategory::InvalidResponse);
    assert_eq!(transport.requests().len(), 2);
}

struct FakeVault;
struct MissingVault;

#[async_trait]
impl GenerationProviderCredentialVaultInterface for FakeVault {
    async fn save_generation_provider_credential(
        &self,
        _id: GenerationProviderCredentialId,
        _secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        Ok(())
    }

    async fn load_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError> {
        GenerationProviderCredentialSecret::new(b"fal-secret".to_vec())
    }

    async fn delete_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        Ok(())
    }
}

#[async_trait]
impl GenerationProviderCredentialVaultInterface for MissingVault {
    async fn save_generation_provider_credential(
        &self,
        _id: GenerationProviderCredentialId,
        _secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        Err(GenerationProviderCredentialVaultError::NotFound)
    }

    async fn load_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError> {
        Err(GenerationProviderCredentialVaultError::NotFound)
    }

    async fn delete_generation_provider_credential(
        &self,
        _id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        Err(GenerationProviderCredentialVaultError::NotFound)
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
        assert_eq!(secret.as_bytes(), b"fal-secret");
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
    ) -> Result<FalHttpResponse, FalTransportError> {
        assert_eq!(url, "https://v3b.fal.media/files/test.png");
        self.download.lock().unwrap().take().ok_or(FalTransportError::DownloadRejected)
    }
}

#[derive(Clone)]
struct RecordedRequest {
    url: String,
    json_body: Option<Vec<u8>>,
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
        GenerationProfileId::try_new("image.high_quality_general").unwrap(),
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

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = image::DynamicImage::new_rgba8(width, height);
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
    cursor.into_inner()
}
