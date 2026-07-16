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
    TextToImageProviderRouteInterface, TextToImageProviderRouteRequest,
};
use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
};
use image::GenericImageView;
use nodes::{
    GeneratedImagePayload, GenerationProfileUnavailableReason, NodeCapabilityDeclaredMediaFacts,
    NodeCapabilityMediaContentDigest, NodeCapabilityMediaSourceLease,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::fal::{
    FalGenerationProviderAccount, FalHttpResponse, FalHttpTransportInterface, FalQueueMethod,
    FalQueueRequest, FalTransportError, ReqwestFalHttpTransport, status_is_success,
};
use crate::credential_repository::{
    GenerationProviderCredentialId, GenerationProviderCredentialRepositoryError,
    GenerationProviderCredentialRepositoryInterface,
};
use wire::{
    FalImageResult, FalStatusResponseDto, FalSubmitResponseDto, FalTextToImageRequestDto,
    FalTextToImageResultDto, aspect_ratio, literal_text, valid_request_id, validated_result,
};

const QUEUE_ROOT: &str = "https://queue.fal.run";
const OPERATION_DEADLINE: Duration = Duration::from_secs(180);
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const POLL_DELAY: Duration = Duration::from_millis(500);
const ROUTE_ID: &str = "fal.text_to_image";
const NATIVE_MODEL_ID: FalTextToImageModelId =
    FalTextToImageModelId("fal-ai/flux-pro/kontext/text-to-image");

#[derive(Clone, Copy)]
struct FalTextToImageModelId(&'static str);

impl FalTextToImageModelId {
    const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Exact Fal queue implementation of the frozen text-to-image route.
pub struct FalTextToImageProviderRouteImpl {
    account: FalGenerationProviderAccount,
    native_model_id: FalTextToImageModelId,
    transport: Arc<dyn FalHttpTransportInterface>,
    poll_delay: Duration,
}

impl FalTextToImageProviderRouteImpl {
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
            native_model_id: NATIVE_MODEL_ID,
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
            native_model_id: NATIVE_MODEL_ID,
            transport,
            poll_delay: Duration::ZERO,
        }
    }

    async fn execute(
        &self,
        request: TextToImageProviderRouteRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        validate_context(&request, false)?;
        let deadline =
            request.context().deadline.monotonic_instant().min(Instant::now() + OPERATION_DEADLINE);
        let body = serde_json::to_vec(&FalTextToImageRequestDto {
            prompt: literal_text(request.prompt()).map_err(|_| {
                failure(NodeCapabilityProviderFailureCategory::InvalidSemanticRequest, false)
            })?,
            aspect_ratio: aspect_ratio(request.aspect_ratio()),
            num_images: 1,
            output_format: "png",
            safety_tolerance: "2",
            enhance_prompt: false,
        })
        .map_err(|_| {
            failure(NodeCapabilityProviderFailureCategory::InvalidSemanticRequest, false)
        })?;
        let request_id = self.submit(body, deadline).await?;
        self.poll(&request, &request_id, deadline).await?;
        let result = self.result(&request_id, deadline).await?;
        self.download(result, deadline).await
    }

    async fn submit(
        &self,
        body: Vec<u8>,
        deadline: Instant,
    ) -> Result<String, NodeCapabilityProviderFailure> {
        let response = self
            .authenticated(
                FalQueueMethod::Post,
                format!("{QUEUE_ROOT}/{}", self.native_model_id.as_str()),
                Some(body),
                deadline,
                false,
            )
            .await?;
        ensure_success(response.status, QueuePhase::Submit)?;
        ensure_json(&response)?;
        let response: FalSubmitResponseDto = parse_json(&response.body)?;
        if !valid_request_id(&response.request_id) {
            return Err(failure(NodeCapabilityProviderFailureCategory::InvalidResponse, false));
        }
        Ok(response.request_id)
    }

    async fn poll(
        &self,
        request: &TextToImageProviderRouteRequest,
        request_id: &str,
        deadline: Instant,
    ) -> Result<(), NodeCapabilityProviderFailure> {
        loop {
            validate_context(request, true)?;
            if Instant::now() >= deadline {
                return Err(failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded, true));
            }
            let url = format!(
                "{QUEUE_ROOT}/{}/requests/{request_id}/status",
                self.native_model_id.as_str()
            );
            let response =
                self.authenticated(FalQueueMethod::Get, url, None, deadline, true).await?;
            ensure_success(response.status, QueuePhase::Accepted)?;
            ensure_json(&response)?;
            match parse_json::<FalStatusResponseDto>(&response.body)?.status.as_str() {
                "COMPLETED" => return Ok(()),
                "IN_QUEUE" | "IN_PROGRESS" => {
                    tokio::time::sleep(self.poll_delay).await;
                }
                "FAILED" | "CANCELLED" => {
                    return Err(failure(
                        NodeCapabilityProviderFailureCategory::ProviderRejected,
                        false,
                    ));
                }
                _ => {
                    return Err(failure(
                        NodeCapabilityProviderFailureCategory::InvalidResponse,
                        false,
                    ));
                }
            }
        }
    }

    async fn result(
        &self,
        request_id: &str,
        deadline: Instant,
    ) -> Result<FalImageResult, NodeCapabilityProviderFailure> {
        let url = format!("{QUEUE_ROOT}/{}/requests/{request_id}", self.native_model_id.as_str());
        let response = self.authenticated(FalQueueMethod::Get, url, None, deadline, true).await?;
        ensure_success(response.status, QueuePhase::Accepted)?;
        ensure_json(&response)?;
        validated_result(parse_json::<FalTextToImageResultDto>(&response.body)?)
            .map_err(|_| invalid_response())
    }

    async fn download(
        &self,
        result: FalImageResult,
        deadline: Instant,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        let response = self
            .transport
            .download_media(&result.url, deadline, MAX_IMAGE_BYTES)
            .await
            .map_err(translate_transport)?;
        if !status_is_success(response.status)
            || response.content_type.as_deref() != Some("image/png")
            || response.body.is_empty()
        {
            return Err(failure(NodeCapabilityProviderFailureCategory::DownloadRejected, false));
        }
        let decoded = image::load_from_memory_with_format(&response.body, image::ImageFormat::Png)
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::DownloadRejected, false))?;
        if decoded.dimensions() != (result.width, result.height) {
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
        let facts = NodeCapabilityDeclaredMediaFacts::try_image(result.width, result.height)
            .map_err(|_| invalid_response())?;
        GeneratedImagePayload::try_new(facts, source).map_err(|_| invalid_response())
    }

    async fn authenticated(
        &self,
        method: FalQueueMethod,
        url: String,
        json_body: Option<Vec<u8>>,
        deadline: Instant,
        submission_already_accepted: bool,
    ) -> Result<FalHttpResponse, NodeCapabilityProviderFailure> {
        let secret = self.account.materialize().await.map_err(credential_failure)?;
        self.transport
            .send_queue_request(
                FalQueueRequest { method, url, json_body, deadline, submission_already_accepted },
                &secret,
            )
            .await
            .map_err(translate_transport)
    }
}

#[async_trait]
impl TextToImageProviderRouteInterface for FalTextToImageProviderRouteImpl {
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

    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRouteRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        self.execute(request).await
    }
}

#[derive(Clone, Copy)]
enum QueuePhase {
    Submit,
    Accepted,
}

fn validate_context(
    request: &TextToImageProviderRouteRequest,
    submission_was_accepted: bool,
) -> Result<(), NodeCapabilityProviderFailure> {
    if request.context().cancellation.is_cancelled()
        || request.context().deadline.is_reached_at(Instant::now())
    {
        Err(failure(
            NodeCapabilityProviderFailureCategory::DeadlineExceeded,
            submission_was_accepted,
        ))
    } else {
        Ok(())
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, NodeCapabilityProviderFailure> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}

fn ensure_success(status: u16, phase: QueuePhase) -> Result<(), NodeCapabilityProviderFailure> {
    if status_is_success(status) {
        return Ok(());
    }
    let category = match status {
        400 if matches!(phase, QueuePhase::Submit) => {
            NodeCapabilityProviderFailureCategory::InvalidSemanticRequest
        }
        401 => NodeCapabilityProviderFailureCategory::AuthenticationFailed,
        403 => NodeCapabilityProviderFailureCategory::PermissionDenied,
        429 => NodeCapabilityProviderFailureCategory::RateLimited,
        500..=599 => NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        _ => NodeCapabilityProviderFailureCategory::ProviderRejected,
    };
    Err(failure(category, false))
}

fn ensure_json(response: &FalHttpResponse) -> Result<(), NodeCapabilityProviderFailure> {
    if response
        .content_type
        .as_deref()
        .is_some_and(|value| value == "application/json" || value.starts_with("application/json;"))
    {
        Ok(())
    } else {
        Err(invalid_response())
    }
}

fn credential_failure(
    error: GenerationProviderCredentialRepositoryError,
) -> NodeCapabilityProviderFailure {
    match error {
        GenerationProviderCredentialRepositoryError::NotFound
        | GenerationProviderCredentialRepositoryError::InvalidCredential => {
            failure(NodeCapabilityProviderFailureCategory::AuthenticationFailed, false)
        }
        GenerationProviderCredentialRepositoryError::PermissionDenied
        | GenerationProviderCredentialRepositoryError::Unavailable => {
            failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable, false)
        }
    }
}

fn translate_transport(error: FalTransportError) -> NodeCapabilityProviderFailure {
    let category = match error {
        FalTransportError::DeadlineBeforeSubmission => {
            NodeCapabilityProviderFailureCategory::DeadlineExceeded
        }
        FalTransportError::DeadlineAfterSubmission | FalTransportError::NetworkAfterSubmission => {
            NodeCapabilityProviderFailureCategory::AmbiguousSubmission
        }
        FalTransportError::NetworkBeforeSubmission => {
            NodeCapabilityProviderFailureCategory::ProviderUnavailable
        }
        FalTransportError::ResponseTooLarge => {
            NodeCapabilityProviderFailureCategory::InvalidResponse
        }
        FalTransportError::DownloadRejected => {
            NodeCapabilityProviderFailureCategory::DownloadRejected
        }
        FalTransportError::InvalidCredential => {
            NodeCapabilityProviderFailureCategory::AuthenticationFailed
        }
    };
    failure(category, false)
}

fn invalid_response() -> NodeCapabilityProviderFailure {
    failure(NodeCapabilityProviderFailureCategory::InvalidResponse, false)
}

fn failure(
    category: NodeCapabilityProviderFailureCategory,
    submission_was_accepted: bool,
) -> NodeCapabilityProviderFailure {
    NodeCapabilityProviderFailure::without_retry_at(category, submission_was_accepted)
}
