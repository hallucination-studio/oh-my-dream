use std::time::Instant;

use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
    WorkflowNodeExecutionContext,
};
use nodes::NodeCapabilityMediaMimeType;
use serde::Deserialize;

use crate::credential_vault::GenerationProviderCredentialVaultError;

use super::super::fal::{FalHttpResponse, FalTransportError, status_is_success};

pub(super) fn image_mime(
    value: NodeCapabilityMediaMimeType,
) -> Result<&'static str, NodeCapabilityProviderFailure> {
    match value {
        NodeCapabilityMediaMimeType::ImagePng => Ok("image/png"),
        NodeCapabilityMediaMimeType::ImageJpeg => Ok("image/jpeg"),
        NodeCapabilityMediaMimeType::ImageWebp => Ok("image/webp"),
        _ => Err(invalid_response()),
    }
}

pub(super) fn valid_mp4(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes[4..8] == *b"ftyp"
}

pub(super) fn validate_context(
    context: &WorkflowNodeExecutionContext,
    accepted: bool,
) -> Result<(), NodeCapabilityProviderFailure> {
    if context.cancellation.is_cancelled() || context.deadline.is_reached_at(Instant::now()) {
        Err(failure(NodeCapabilityProviderFailureCategory::DeadlineExceeded, accepted))
    } else {
        Ok(())
    }
}

pub(super) fn ensure_success(
    status: u16,
    accepted: bool,
) -> Result<(), NodeCapabilityProviderFailure> {
    if status_is_success(status) {
        return Ok(());
    }
    let category = match status {
        400 if !accepted => NodeCapabilityProviderFailureCategory::InvalidSemanticRequest,
        401 => NodeCapabilityProviderFailureCategory::AuthenticationFailed,
        403 => NodeCapabilityProviderFailureCategory::PermissionDenied,
        429 => NodeCapabilityProviderFailureCategory::RateLimited,
        500..=599 => NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        _ => NodeCapabilityProviderFailureCategory::ProviderRejected,
    };
    Err(failure(category, false))
}

pub(super) fn ensure_json(response: &FalHttpResponse) -> Result<(), NodeCapabilityProviderFailure> {
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

pub(super) fn parse_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, NodeCapabilityProviderFailure> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}

pub(super) fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn credential_failure(
    error: GenerationProviderCredentialVaultError,
) -> NodeCapabilityProviderFailure {
    match error {
        GenerationProviderCredentialVaultError::NotFound
        | GenerationProviderCredentialVaultError::Denied
        | GenerationProviderCredentialVaultError::InvalidCredential => {
            failure(NodeCapabilityProviderFailureCategory::AuthenticationFailed, false)
        }
        GenerationProviderCredentialVaultError::Unavailable => {
            failure(NodeCapabilityProviderFailureCategory::ProviderUnavailable, false)
        }
    }
}

pub(super) fn translate_transport(error: FalTransportError) -> NodeCapabilityProviderFailure {
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

pub(super) fn invalid_response() -> NodeCapabilityProviderFailure {
    failure(NodeCapabilityProviderFailureCategory::InvalidResponse, false)
}

pub(super) fn failure(
    category: NodeCapabilityProviderFailureCategory,
    accepted: bool,
) -> NodeCapabilityProviderFailure {
    NodeCapabilityProviderFailure::without_retry_at(category, accepted)
}
