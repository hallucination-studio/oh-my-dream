mod support;
#[cfg(test)]
mod tests;
mod wire;

use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use backends::provider_routing::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    ImageToVideoProviderRouteInterface, ImageToVideoProviderRouteRequest,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
    WorkflowNodeExecutionContext,
};
use nodes::{
    GeneratedVideoPayload, GenerationProfileUnavailableReason, ImageToVideoDurationSeconds,
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaContentDigest,
    NodeCapabilityMediaSourceLease,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use super::fal::{
    FalGenerationProviderAccount, FalHttpResponse, FalHttpTransportInterface, FalQueueMethod,
    FalQueueRequest, ReqwestFalHttpTransport, status_is_success,
};
use crate::credential_repository::{
    GenerationProviderCredentialId, GenerationProviderCredentialRepositoryError,
    GenerationProviderCredentialRepositoryInterface,
};
use support::*;
use wire::{FalImageToVideoRequestDto, FalVideoDto, FalVideoResultDto};

const QUEUE_ROOT: &str = "https://queue.fal.run";
const OPERATION_DEADLINE: Duration = Duration::from_secs(900);
const POLL_DELAY: Duration = Duration::from_millis(500);
const MAX_VIDEO_BYTES: usize = 512 * 1024 * 1024;
const ROUTE_ID: &str = "fal.image_to_video";
const NATIVE_MODEL_ID: FalImageToVideoModelId =
    FalImageToVideoModelId("fal-ai/kling-video/v3/standard/image-to-video");

#[derive(Clone, Copy)]
struct FalImageToVideoModelId(&'static str);

impl FalImageToVideoModelId {
    const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Exact Fal queue implementation of the frozen image-to-video route.
pub struct FalImageToVideoProviderRouteImpl {
    account: FalGenerationProviderAccount,
    transport: Arc<dyn FalHttpTransportInterface>,
    poll_delay: Duration,
}

impl FalImageToVideoProviderRouteImpl {
    /// Constructs the shipped Fal route with a focused credential repository.
    pub fn try_new(
        credential_id: GenerationProviderCredentialId,
        repository: Arc<dyn GenerationProviderCredentialRepositoryInterface>,
    ) -> Result<Self, NodeCapabilityProviderFailure> {
        let transport = ReqwestFalHttpTransport::try_new().map_err(|_| {
            failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable, false)
        })?;
        Ok(Self {
            account: FalGenerationProviderAccount::new(credential_id, repository),
            transport: Arc::new(transport),
            poll_delay: POLL_DELAY,
        })
    }

    #[cfg(test)]
    fn with_transport(
        credential_id: GenerationProviderCredentialId,
        repository: Arc<dyn GenerationProviderCredentialRepositoryInterface>,
        transport: Arc<dyn FalHttpTransportInterface>,
    ) -> Self {
        Self {
            account: FalGenerationProviderAccount::new(credential_id, repository),
            transport,
            poll_delay: Duration::ZERO,
        }
    }

    async fn execute(
        &self,
        request: ImageToVideoProviderRouteRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure> {
        let (context, image, prompt, duration) = request.into_parts();
        validate_context(&context, false)?;
        let deadline =
            context.deadline.monotonic_instant().min(Instant::now() + OPERATION_DEADLINE);
        let facts = image.facts();
        let NodeCapabilityDeclaredMediaFacts::Image(image_facts) = facts else {
            return Err(invalid_response());
        };
        let mime = image_mime(image.mime_type())?;
        let source = image.into_source();
        let expected_length = source.byte_length();
        let expected_digest = source.digest();
        let stream = source.try_take_stream().map_err(|_| invalid_response())?;
        let mut image_bytes = Vec::new();
        stream
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut image_bytes)
            .await
            .map_err(|_| invalid_response())?;
        let actual_digest: [u8; 32] = Sha256::digest(&image_bytes).into();
        if image_bytes.is_empty()
            || image_bytes.len() > 32 * 1024 * 1024
            || image_bytes.len() as u64 != expected_length
            || actual_digest != expected_digest.as_bytes()
        {
            return Err(invalid_response());
        }
        let start_image_url = format!("data:{mime};base64,{}", STANDARD.encode(image_bytes));
        let body = serde_json::to_vec(&FalImageToVideoRequestDto {
            start_image_url,
            prompt: wire::prompt(prompt.as_ref()).map_err(|_| {
                failure(NodeCapabilityProviderFailureCategory::InvalidSemanticRequest, false)
            })?,
            duration: wire::duration(duration),
            generate_audio: false,
        })
        .map_err(|_| invalid_response())?;
        let request_id = self.submit(body, deadline).await?;
        self.poll(&context, &request_id, deadline).await?;
        let result = self.result(&request_id, deadline).await?;
        self.download(result, image_facts.width(), image_facts.height(), duration, deadline).await
    }

    async fn submit(
        &self,
        body: Vec<u8>,
        deadline: Instant,
    ) -> Result<String, NodeCapabilityProviderFailure> {
        let response = self
            .authenticated(FalQueueMethod::Post, submit_url(), Some(body), deadline, false)
            .await?;
        ensure_success(response.status, false)?;
        ensure_json(&response)?;
        let response: SubmitDto = parse_json(&response.body)?;
        if !valid_request_id(&response.request_id) {
            return Err(invalid_response());
        }
        Ok(response.request_id)
    }

    async fn poll(
        &self,
        context: &WorkflowNodeExecutionContext,
        request_id: &str,
        deadline: Instant,
    ) -> Result<(), NodeCapabilityProviderFailure> {
        loop {
            validate_context(context, true)?;
            if Instant::now() >= deadline {
                return Err(failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded, true));
            }
            let response = self
                .authenticated(FalQueueMethod::Get, status_url(request_id), None, deadline, true)
                .await?;
            ensure_success(response.status, true)?;
            ensure_json(&response)?;
            match parse_json::<StatusDto>(&response.body)?.status.as_str() {
                "COMPLETED" => return Ok(()),
                "IN_QUEUE" | "IN_PROGRESS" => tokio::time::sleep(self.poll_delay).await,
                "FAILED" | "CANCELLED" => {
                    return Err(failure(
                        NodeCapabilityProviderFailureCategory::ProviderRejected,
                        false,
                    ));
                }
                _ => return Err(invalid_response()),
            }
        }
    }

    async fn result(
        &self,
        request_id: &str,
        deadline: Instant,
    ) -> Result<FalVideoDto, NodeCapabilityProviderFailure> {
        let response = self
            .authenticated(FalQueueMethod::Get, result_url(request_id), None, deadline, true)
            .await?;
        ensure_success(response.status, true)?;
        ensure_json(&response)?;
        wire::validate_result(parse_json::<FalVideoResultDto>(&response.body)?)
            .map_err(|_| invalid_response())
    }

    async fn download(
        &self,
        result: FalVideoDto,
        width: u32,
        height: u32,
        duration: ImageToVideoDurationSeconds,
        deadline: Instant,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure> {
        let response = self
            .transport
            .download_media(&result.url, deadline, MAX_VIDEO_BYTES)
            .await
            .map_err(translate_transport)?;
        if !status_is_success(response.status)
            || response.content_type.as_deref() != Some("video/mp4")
            || !valid_mp4(&response.body)
            || result.file_size.is_some_and(|size| size != response.body.len() as u64)
        {
            return Err(failure(NodeCapabilityProviderFailureCategory::DownloadRejected, false));
        }
        let digest =
            NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&response.body).into());
        let source = NodeCapabilityMediaSourceLease::try_new(
            response.body.len() as u64,
            digest,
            deadline,
            Box::pin(Cursor::new(response.body)),
        )
        .map_err(|_| invalid_response())?;
        let duration_ms = match duration {
            ImageToVideoDurationSeconds::Five => 5_000,
            ImageToVideoDurationSeconds::Ten => 10_000,
        };
        let facts = NodeCapabilityDeclaredMediaFacts::try_video(width, height, duration_ms, false)
            .map_err(|_| invalid_response())?;
        GeneratedVideoPayload::try_new(facts, source).map_err(|_| invalid_response())
    }

    async fn authenticated(
        &self,
        method: FalQueueMethod,
        url: String,
        body: Option<Vec<u8>>,
        deadline: Instant,
        accepted: bool,
    ) -> Result<FalHttpResponse, NodeCapabilityProviderFailure> {
        let secret = self.account.materialize().await.map_err(credential_failure)?;
        self.transport
            .send_queue_request(
                FalQueueRequest {
                    method,
                    url,
                    json_body: body,
                    deadline,
                    submission_already_accepted: accepted,
                },
                &secret,
            )
            .await
            .map_err(translate_transport)
    }
}

#[async_trait]
impl ImageToVideoProviderRouteInterface for FalImageToVideoProviderRouteImpl {
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId {
        GenerationProviderRouteId::new(ROUTE_ID)
    }

    async fn observe_provider_route_availability(&self) -> GenerationProviderRouteAvailability {
        match self.account.credential_state().await {
            Ok(()) => GenerationProviderRouteAvailability::Available,
            Err(
                GenerationProviderCredentialRepositoryError::NotFound
                | GenerationProviderCredentialRepositoryError::InvalidCredential,
            ) => GenerationProviderRouteAvailability::Unavailable {
                reason: GenerationProfileUnavailableReason::AuthenticationRequired,
                retry_after: None,
            },
            Err(
                GenerationProviderCredentialRepositoryError::PermissionDenied
                | GenerationProviderCredentialRepositoryError::Unavailable,
            ) => GenerationProviderRouteAvailability::Unavailable {
                reason: GenerationProfileUnavailableReason::ProviderUnavailable,
                retry_after: None,
            },
        }
    }

    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRouteRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure> {
        self.execute(request).await
    }
}

#[derive(Deserialize)]
struct SubmitDto {
    request_id: String,
}

#[derive(Deserialize)]
struct StatusDto {
    status: String,
}

fn submit_url() -> String {
    format!("{QUEUE_ROOT}/{}", NATIVE_MODEL_ID.as_str())
}

fn status_url(request_id: &str) -> String {
    format!("{}/{}/requests/{request_id}/status", QUEUE_ROOT, NATIVE_MODEL_ID.as_str())
}

fn result_url(request_id: &str) -> String {
    format!("{}/{}/requests/{request_id}", QUEUE_ROOT, NATIVE_MODEL_ID.as_str())
}
