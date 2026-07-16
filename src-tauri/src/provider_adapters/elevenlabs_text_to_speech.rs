mod mp3;
#[cfg(test)]
mod tests;

use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use backends::provider_routing::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    TextToSpeechProviderRouteInterface, TextToSpeechProviderRouteRequest,
};
use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory, WorkflowTextPart,
    WorkflowTextValue,
};
use nodes::{
    GenerationProfileAvailabilityIndeterminateReason, GenerationProfileUnavailableReason,
    NodeCapabilityMediaContentDigest, NodeCapabilityMediaSourceLease, SynthesizedSpeechPayload,
};
use reqwest::header;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::credential_vault::{
    GenerationProviderCredentialId, GenerationProviderCredentialSecret,
    GenerationProviderCredentialVaultError, GenerationProviderCredentialVaultInterface,
};

const API_ROOT: &str = "https://api.elevenlabs.io";
const MODEL_ID: &str = "eleven_multilingual_v2";
const ROUTE_ID: &str = "elevenlabs.text_to_speech";
const MAX_AUDIO_BYTES: usize = 64 * 1024 * 1024;
const OPERATION_DEADLINE: Duration = Duration::from_secs(120);

/// Validated non-secret ElevenLabs voice configuration.
#[derive(Clone)]
pub struct ElevenLabsVoiceId(String);

impl ElevenLabsVoiceId {
    /// Validates one path-safe vendor voice identifier.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ElevenLabsVoiceIdError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ElevenLabsVoiceIdError);
        }
        Ok(Self(value))
    }
}

/// Returned when an ElevenLabs voice ID is empty or not path-safe.
#[derive(Debug, thiserror::Error)]
#[error("invalid ElevenLabs voice ID")]
pub struct ElevenLabsVoiceIdError;

/// Exact ElevenLabs implementation of the frozen text-to-speech route.
pub struct ElevenLabsTextToSpeechProviderRouteImpl {
    credential_id: GenerationProviderCredentialId,
    vault: Arc<dyn GenerationProviderCredentialVaultInterface>,
    voice_id: ElevenLabsVoiceId,
    client: reqwest::Client,
}

impl ElevenLabsTextToSpeechProviderRouteImpl {
    /// Constructs the shipped route from non-secret voice and opaque credential configuration.
    pub fn try_new(
        credential_id: GenerationProviderCredentialId,
        vault: Arc<dyn GenerationProviderCredentialVaultInterface>,
        voice_id: ElevenLabsVoiceId,
    ) -> Result<Self, NodeCapabilityProviderFailure> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable))?;
        Ok(Self { credential_id, vault, voice_id, client })
    }

    async fn execute(
        &self,
        request: TextToSpeechProviderRouteRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure> {
        let (context, text) = request.into_parts();
        if context.cancellation.is_cancelled() || context.deadline.is_reached_at(Instant::now()) {
            return Err(failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded));
        }
        let deadline =
            context.deadline.monotonic_instant().min(Instant::now() + OPERATION_DEADLINE);
        let body = serde_json::to_vec(&RequestDto {
            text: literal_text(&text).map_err(|_| {
                failure(NodeCapabilityProviderFailureCategory::InvalidSemanticRequest)
            })?,
            model_id: MODEL_ID,
        })
        .map_err(|_| failure(NodeCapabilityProviderFailureCategory::InvalidSemanticRequest))?;
        let secret = self
            .vault
            .load_generation_provider_credential(&self.credential_id)
            .await
            .map_err(credential_failure)?;
        let response = self.send(body, &secret, deadline).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(http_failure(status.as_u16()));
        }
        if response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| value != "audio/mpeg")
        {
            return Err(failure(NodeCapabilityProviderFailureCategory::InvalidResponse));
        }
        let bytes = read_bounded(response, deadline).await?;
        let facts = mp3::inspect(&bytes)
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::InvalidResponse))?;
        let digest = NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&bytes).into());
        let source = NodeCapabilityMediaSourceLease::try_new(
            bytes.len() as u64,
            digest,
            deadline,
            Box::pin(Cursor::new(bytes)),
        )
        .map_err(|_| failure(NodeCapabilityProviderFailureCategory::InvalidResponse))?;
        SynthesizedSpeechPayload::try_new(facts, source)
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::InvalidResponse))
    }

    async fn send(
        &self,
        body: Vec<u8>,
        secret: &GenerationProviderCredentialSecret,
        deadline: Instant,
    ) -> Result<reqwest::Response, NodeCapabilityProviderFailure> {
        let mut header_bytes = secret.as_bytes().to_vec();
        let header_result = header::HeaderValue::from_bytes(&header_bytes);
        header_bytes.fill(0);
        let mut credential = header_result
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::AuthenticationFailed))?;
        credential.set_sensitive(true);
        let timeout = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded))?;
        self.client
            .post(endpoint(&self.voice_id.0))
            .header("xi-api-key", credential)
            .header(header::CONTENT_TYPE, "application/json")
            .timeout(timeout)
            .body(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded)
                } else {
                    failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable)
                }
            })
    }
}

#[async_trait]
impl TextToSpeechProviderRouteInterface for ElevenLabsTextToSpeechProviderRouteImpl {
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId {
        GenerationProviderRouteId::new(ROUTE_ID)
    }

    async fn observe_provider_route_availability(&self) -> GenerationProviderRouteAvailability {
        match self.vault.load_generation_provider_credential(&self.credential_id).await {
            Ok(secret) => {
                drop(secret);
                GenerationProviderRouteAvailability::Available
            }
            Err(
                GenerationProviderCredentialVaultError::NotFound
                | GenerationProviderCredentialVaultError::Denied
                | GenerationProviderCredentialVaultError::InvalidCredential,
            ) => GenerationProviderRouteAvailability::Unavailable {
                reason: GenerationProfileUnavailableReason::AuthenticationRequired,
                retry_after: None,
            },
            Err(GenerationProviderCredentialVaultError::Unavailable) => {
                GenerationProviderRouteAvailability::Indeterminate {
                    reason: GenerationProfileAvailabilityIndeterminateReason::NetworkOffline,
                }
            }
        }
    }

    async fn synthesize_speech_from_text(
        &self,
        request: TextToSpeechProviderRouteRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure> {
        self.execute(request).await
    }
}

#[derive(Serialize)]
struct RequestDto<'a> {
    text: String,
    model_id: &'a str,
}

fn endpoint(voice_id: &str) -> String {
    format!("{API_ROOT}/v1/text-to-speech/{voice_id}?output_format=mp3_44100_128")
}

fn literal_text(value: &WorkflowTextValue) -> Result<String, ()> {
    let mut text = String::new();
    for part in value.parts() {
        match part {
            WorkflowTextPart::Literal(value) => text.push_str(value),
            WorkflowTextPart::InputItemReference(_) => return Err(()),
        }
    }
    if text.is_empty() { Err(()) } else { Ok(text) }
}

async fn read_bounded(
    mut response: reqwest::Response,
    deadline: Instant,
) -> Result<Vec<u8>, NodeCapabilityProviderFailure> {
    let mut bytes = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded));
        }
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failure(NodeCapabilityProviderFailureCategory::InvalidResponse))?
        else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_AUDIO_BYTES {
            return Err(failure(NodeCapabilityProviderFailureCategory::InvalidResponse));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        Err(failure(NodeCapabilityProviderFailureCategory::InvalidResponse))
    } else {
        Ok(bytes)
    }
}

fn http_failure(status: u16) -> NodeCapabilityProviderFailure {
    let category = match status {
        400 | 422 => NodeCapabilityProviderFailureCategory::InvalidSemanticRequest,
        401 => NodeCapabilityProviderFailureCategory::AuthenticationFailed,
        403 => NodeCapabilityProviderFailureCategory::PermissionDenied,
        429 => NodeCapabilityProviderFailureCategory::RateLimited,
        500..=599 => NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        _ => NodeCapabilityProviderFailureCategory::ProviderRejected,
    };
    failure(category)
}

fn credential_failure(
    error: GenerationProviderCredentialVaultError,
) -> NodeCapabilityProviderFailure {
    match error {
        GenerationProviderCredentialVaultError::NotFound
        | GenerationProviderCredentialVaultError::Denied
        | GenerationProviderCredentialVaultError::InvalidCredential => {
            failure(NodeCapabilityProviderFailureCategory::AuthenticationFailed)
        }
        GenerationProviderCredentialVaultError::Unavailable => {
            failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable)
        }
    }
}

fn failure(category: NodeCapabilityProviderFailureCategory) -> NodeCapabilityProviderFailure {
    NodeCapabilityProviderFailure::without_retry_at(category, false)
}
