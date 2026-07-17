use engine::node_capability::{
    NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityGenerationTaskStartFailure,
    NodeCapabilityMediaFailure, NodeCapabilityParameterKey, NodeCapabilityProviderFailureCategory,
    NodeCapabilityReadinessCategory, NodeCapabilityReadinessIssue, NodeCapabilityReadinessTarget,
    WorkflowDataType, WorkflowManagedAssetIdBoundaryValue,
};
use engine::workflow::{WorkflowApplicationError, WorkflowGenerationTaskFailure};
use serde::{Deserialize, Serialize};

use super::super::super::persistence;

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum StagePayload {
    ResolveInputs,
    StartGenerationTask,
    CallProvider,
    ValidateProviderResult,
    WriteManagedMedia,
    AssembleOutputs,
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum TargetPayload {
    Capability,
    Parameter(String),
    Input(String),
    Output(String),
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum ExecutionSourcePayload {
    InvalidCapabilityInvocation,
    InvalidCapabilityResult,
    Readiness(ReadinessPayload),
    Provider {
        category: ProviderCategoryPayload,
        retryable: bool,
        safe_retry_after_nanos: Option<u64>,
    },
    Media(MediaFailurePayload),
    GenerationTaskStart(GenerationTaskStartFailurePayload),
    Cancelled,
    DeadlineExceeded,
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum GenerationTaskStartFailurePayload {
    InvalidRequest,
    Conflict,
    Unavailable,
    Cancelled,
    DeadlineExceeded,
    Persistence,
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum GenerationTaskFailurePayload {
    InvalidRequest,
    Authentication,
    PermissionDenied,
    ContentPolicy,
    RateLimited,
    ProviderUnavailable,
    Timeout,
    ProviderRejected,
    InvalidProviderResponse,
    AmbiguousSubmission,
    InputAssetUnavailable,
    OutputAssetImport,
    Internal,
    GenerationTaskCancelled,
}

macro_rules! generation_task_failure_mapping {
    ($value:expr, $source:ident, $target:ident) => {
        match $value {
            $source::InvalidRequest => $target::InvalidRequest,
            $source::Authentication => $target::Authentication,
            $source::PermissionDenied => $target::PermissionDenied,
            $source::ContentPolicy => $target::ContentPolicy,
            $source::RateLimited => $target::RateLimited,
            $source::ProviderUnavailable => $target::ProviderUnavailable,
            $source::Timeout => $target::Timeout,
            $source::ProviderRejected => $target::ProviderRejected,
            $source::InvalidProviderResponse => $target::InvalidProviderResponse,
            $source::AmbiguousSubmission => $target::AmbiguousSubmission,
            $source::InputAssetUnavailable => $target::InputAssetUnavailable,
            $source::OutputAssetImport => $target::OutputAssetImport,
            $source::Internal => $target::Internal,
            $source::GenerationTaskCancelled => $target::GenerationTaskCancelled,
        }
    };
}

pub(super) fn encode_generation_task_failure(
    value: WorkflowGenerationTaskFailure,
) -> GenerationTaskFailurePayload {
    generation_task_failure_mapping!(
        value,
        WorkflowGenerationTaskFailure,
        GenerationTaskFailurePayload
    )
}

pub(super) fn decode_generation_task_failure(
    value: GenerationTaskFailurePayload,
) -> WorkflowGenerationTaskFailure {
    generation_task_failure_mapping!(
        value,
        GenerationTaskFailurePayload,
        WorkflowGenerationTaskFailure
    )
}

pub(super) fn encode_generation_task_start_failure(
    value: NodeCapabilityGenerationTaskStartFailure,
) -> GenerationTaskStartFailurePayload {
    match value {
        NodeCapabilityGenerationTaskStartFailure::InvalidRequest => {
            GenerationTaskStartFailurePayload::InvalidRequest
        }
        NodeCapabilityGenerationTaskStartFailure::Conflict => {
            GenerationTaskStartFailurePayload::Conflict
        }
        NodeCapabilityGenerationTaskStartFailure::Unavailable => {
            GenerationTaskStartFailurePayload::Unavailable
        }
        NodeCapabilityGenerationTaskStartFailure::Cancelled => {
            GenerationTaskStartFailurePayload::Cancelled
        }
        NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded => {
            GenerationTaskStartFailurePayload::DeadlineExceeded
        }
        NodeCapabilityGenerationTaskStartFailure::Persistence => {
            GenerationTaskStartFailurePayload::Persistence
        }
    }
}

pub(super) fn decode_generation_task_start_failure(
    value: GenerationTaskStartFailurePayload,
) -> NodeCapabilityGenerationTaskStartFailure {
    match value {
        GenerationTaskStartFailurePayload::InvalidRequest => {
            NodeCapabilityGenerationTaskStartFailure::InvalidRequest
        }
        GenerationTaskStartFailurePayload::Conflict => {
            NodeCapabilityGenerationTaskStartFailure::Conflict
        }
        GenerationTaskStartFailurePayload::Unavailable => {
            NodeCapabilityGenerationTaskStartFailure::Unavailable
        }
        GenerationTaskStartFailurePayload::Cancelled => {
            NodeCapabilityGenerationTaskStartFailure::Cancelled
        }
        GenerationTaskStartFailurePayload::DeadlineExceeded => {
            NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded
        }
        GenerationTaskStartFailurePayload::Persistence => {
            NodeCapabilityGenerationTaskStartFailure::Persistence
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) struct ReadinessPayload {
    pub(super) category: ReadinessCategoryPayload,
    pub(super) target: ReadinessTargetPayload,
    pub(super) media_kind_mismatch: Option<(DataTypePayload, DataTypePayload)>,
}

#[derive(Serialize, Deserialize)]
pub(super) enum ReadinessTargetPayload {
    Capability,
    ManagedAsset { parameter_key: String, asset_id: [u8; 16] },
    GenerationProfile { parameter_key: String, profile_id: String, version: u32 },
}

#[derive(Serialize, Deserialize)]
pub(super) enum ReadinessCategoryPayload {
    InvalidCapabilityInvocation,
    ManagedAssetUnavailable,
    ManagedAssetKindMismatch,
    ManagedAssetReadinessIndeterminate,
    GenerationProfileIncompatible,
    GenerationProfileUnavailable,
    GenerationProfileAvailabilityIndeterminate,
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum ProviderCategoryPayload {
    InvalidSemanticRequest,
    AuthenticationFailed,
    PermissionDenied,
    ContentPolicyRejected,
    RateLimited,
    ProviderUnavailable,
    DeadlineExceeded,
    ProviderRejected,
    InvalidResponse,
    DownloadRejected,
    AmbiguousSubmission,
}

#[derive(Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum MediaFailurePayload {
    Unavailable,
    KindMismatch { expected: DataTypePayload, observed: DataTypePayload },
    InvalidMedia,
    SizeLimitExceeded,
    DigestMismatch,
    OutputConflict,
    StorageFailed,
    InspectionFailed,
    FinalizationFailed,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(in crate::workflow_storage_adapters::run) enum DataTypePayload {
    Text,
    Image,
    Video,
    Audio,
}

pub(super) fn encode_readiness(value: &NodeCapabilityReadinessIssue) -> ReadinessPayload {
    ReadinessPayload {
        category: match value.category() {
            NodeCapabilityReadinessCategory::InvalidCapabilityInvocation => {
                ReadinessCategoryPayload::InvalidCapabilityInvocation
            }
            NodeCapabilityReadinessCategory::ManagedAssetUnavailable => {
                ReadinessCategoryPayload::ManagedAssetUnavailable
            }
            NodeCapabilityReadinessCategory::ManagedAssetKindMismatch => {
                ReadinessCategoryPayload::ManagedAssetKindMismatch
            }
            NodeCapabilityReadinessCategory::ManagedAssetReadinessIndeterminate => {
                ReadinessCategoryPayload::ManagedAssetReadinessIndeterminate
            }
            NodeCapabilityReadinessCategory::GenerationProfileIncompatible => {
                ReadinessCategoryPayload::GenerationProfileIncompatible
            }
            NodeCapabilityReadinessCategory::GenerationProfileUnavailable => {
                ReadinessCategoryPayload::GenerationProfileUnavailable
            }
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate => {
                ReadinessCategoryPayload::GenerationProfileAvailabilityIndeterminate
            }
        },
        target: encode_readiness_target(value.target()),
        media_kind_mismatch: value
            .media_kind_mismatch()
            .map(|(expected, observed)| (encode_data_type(expected), encode_data_type(observed))),
    }
}

pub(super) fn decode_readiness(
    value: ReadinessPayload,
) -> Result<NodeCapabilityReadinessIssue, WorkflowApplicationError> {
    let category = match value.category {
        ReadinessCategoryPayload::InvalidCapabilityInvocation => {
            NodeCapabilityReadinessCategory::InvalidCapabilityInvocation
        }
        ReadinessCategoryPayload::ManagedAssetUnavailable => {
            NodeCapabilityReadinessCategory::ManagedAssetUnavailable
        }
        ReadinessCategoryPayload::ManagedAssetKindMismatch => {
            NodeCapabilityReadinessCategory::ManagedAssetKindMismatch
        }
        ReadinessCategoryPayload::ManagedAssetReadinessIndeterminate => {
            NodeCapabilityReadinessCategory::ManagedAssetReadinessIndeterminate
        }
        ReadinessCategoryPayload::GenerationProfileIncompatible => {
            NodeCapabilityReadinessCategory::GenerationProfileIncompatible
        }
        ReadinessCategoryPayload::GenerationProfileUnavailable => {
            NodeCapabilityReadinessCategory::GenerationProfileUnavailable
        }
        ReadinessCategoryPayload::GenerationProfileAvailabilityIndeterminate => {
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate
        }
    };
    NodeCapabilityReadinessIssue::try_new(
        category,
        decode_readiness_target(value.target)?,
        value
            .media_kind_mismatch
            .map(|(expected, observed)| (decode_data_type(expected), decode_data_type(observed))),
    )
    .map_err(|_| persistence())
}

fn encode_readiness_target(value: &NodeCapabilityReadinessTarget) -> ReadinessTargetPayload {
    match value {
        NodeCapabilityReadinessTarget::Capability => ReadinessTargetPayload::Capability,
        NodeCapabilityReadinessTarget::ManagedAsset { parameter_key, asset_id } => {
            ReadinessTargetPayload::ManagedAsset {
                parameter_key: parameter_key.as_str().to_owned(),
                asset_id: asset_id.as_bytes(),
            }
        }
        NodeCapabilityReadinessTarget::GenerationProfile {
            parameter_key,
            generation_profile_ref,
        } => ReadinessTargetPayload::GenerationProfile {
            parameter_key: parameter_key.as_str().to_owned(),
            profile_id: generation_profile_ref.profile_id().to_owned(),
            version: generation_profile_ref.version(),
        },
    }
}

fn decode_readiness_target(
    value: ReadinessTargetPayload,
) -> Result<NodeCapabilityReadinessTarget, WorkflowApplicationError> {
    match value {
        ReadinessTargetPayload::Capability => Ok(NodeCapabilityReadinessTarget::Capability),
        ReadinessTargetPayload::ManagedAsset { parameter_key, asset_id } => {
            Ok(NodeCapabilityReadinessTarget::ManagedAsset {
                parameter_key: NodeCapabilityParameterKey::new(parameter_key)
                    .map_err(|_| persistence())?,
                asset_id: WorkflowManagedAssetIdBoundaryValue::from_bytes(asset_id)
                    .map_err(|_| persistence())?,
            })
        }
        ReadinessTargetPayload::GenerationProfile { parameter_key, profile_id, version } => {
            Ok(NodeCapabilityReadinessTarget::GenerationProfile {
                parameter_key: NodeCapabilityParameterKey::new(parameter_key)
                    .map_err(|_| persistence())?,
                generation_profile_ref: NodeCapabilityGenerationProfileRefParameterValue::new(
                    profile_id, version,
                )
                .map_err(|_| persistence())?,
            })
        }
    }
}

pub(super) fn encode_provider_category(
    value: NodeCapabilityProviderFailureCategory,
) -> ProviderCategoryPayload {
    match value {
        NodeCapabilityProviderFailureCategory::InvalidSemanticRequest => {
            ProviderCategoryPayload::InvalidSemanticRequest
        }
        NodeCapabilityProviderFailureCategory::AuthenticationFailed => {
            ProviderCategoryPayload::AuthenticationFailed
        }
        NodeCapabilityProviderFailureCategory::PermissionDenied => {
            ProviderCategoryPayload::PermissionDenied
        }
        NodeCapabilityProviderFailureCategory::ContentPolicyRejected => {
            ProviderCategoryPayload::ContentPolicyRejected
        }
        NodeCapabilityProviderFailureCategory::RateLimited => ProviderCategoryPayload::RateLimited,
        NodeCapabilityProviderFailureCategory::ProviderUnavailable => {
            ProviderCategoryPayload::ProviderUnavailable
        }
        NodeCapabilityProviderFailureCategory::DeadlineExceeded => {
            ProviderCategoryPayload::DeadlineExceeded
        }
        NodeCapabilityProviderFailureCategory::ProviderRejected => {
            ProviderCategoryPayload::ProviderRejected
        }
        NodeCapabilityProviderFailureCategory::InvalidResponse => {
            ProviderCategoryPayload::InvalidResponse
        }
        NodeCapabilityProviderFailureCategory::DownloadRejected => {
            ProviderCategoryPayload::DownloadRejected
        }
        NodeCapabilityProviderFailureCategory::AmbiguousSubmission => {
            ProviderCategoryPayload::AmbiguousSubmission
        }
    }
}

pub(super) fn decode_provider_category(
    value: ProviderCategoryPayload,
) -> NodeCapabilityProviderFailureCategory {
    match value {
        ProviderCategoryPayload::InvalidSemanticRequest => {
            NodeCapabilityProviderFailureCategory::InvalidSemanticRequest
        }
        ProviderCategoryPayload::AuthenticationFailed => {
            NodeCapabilityProviderFailureCategory::AuthenticationFailed
        }
        ProviderCategoryPayload::PermissionDenied => {
            NodeCapabilityProviderFailureCategory::PermissionDenied
        }
        ProviderCategoryPayload::ContentPolicyRejected => {
            NodeCapabilityProviderFailureCategory::ContentPolicyRejected
        }
        ProviderCategoryPayload::RateLimited => NodeCapabilityProviderFailureCategory::RateLimited,
        ProviderCategoryPayload::ProviderUnavailable => {
            NodeCapabilityProviderFailureCategory::ProviderUnavailable
        }
        ProviderCategoryPayload::DeadlineExceeded => {
            NodeCapabilityProviderFailureCategory::DeadlineExceeded
        }
        ProviderCategoryPayload::ProviderRejected => {
            NodeCapabilityProviderFailureCategory::ProviderRejected
        }
        ProviderCategoryPayload::InvalidResponse => {
            NodeCapabilityProviderFailureCategory::InvalidResponse
        }
        ProviderCategoryPayload::DownloadRejected => {
            NodeCapabilityProviderFailureCategory::DownloadRejected
        }
        ProviderCategoryPayload::AmbiguousSubmission => {
            NodeCapabilityProviderFailureCategory::AmbiguousSubmission
        }
    }
}

pub(super) fn encode_media_failure(value: NodeCapabilityMediaFailure) -> MediaFailurePayload {
    match value {
        NodeCapabilityMediaFailure::Unavailable => MediaFailurePayload::Unavailable,
        NodeCapabilityMediaFailure::KindMismatch { expected, observed } => {
            MediaFailurePayload::KindMismatch {
                expected: encode_data_type(expected),
                observed: encode_data_type(observed),
            }
        }
        NodeCapabilityMediaFailure::InvalidMedia => MediaFailurePayload::InvalidMedia,
        NodeCapabilityMediaFailure::SizeLimitExceeded => MediaFailurePayload::SizeLimitExceeded,
        NodeCapabilityMediaFailure::DigestMismatch => MediaFailurePayload::DigestMismatch,
        NodeCapabilityMediaFailure::OutputConflict => MediaFailurePayload::OutputConflict,
        NodeCapabilityMediaFailure::StorageFailed => MediaFailurePayload::StorageFailed,
        NodeCapabilityMediaFailure::InspectionFailed => MediaFailurePayload::InspectionFailed,
        NodeCapabilityMediaFailure::FinalizationFailed => MediaFailurePayload::FinalizationFailed,
    }
}

pub(super) fn decode_media_failure(value: MediaFailurePayload) -> NodeCapabilityMediaFailure {
    match value {
        MediaFailurePayload::Unavailable => NodeCapabilityMediaFailure::Unavailable,
        MediaFailurePayload::KindMismatch { expected, observed } => {
            NodeCapabilityMediaFailure::KindMismatch {
                expected: decode_data_type(expected),
                observed: decode_data_type(observed),
            }
        }
        MediaFailurePayload::InvalidMedia => NodeCapabilityMediaFailure::InvalidMedia,
        MediaFailurePayload::SizeLimitExceeded => NodeCapabilityMediaFailure::SizeLimitExceeded,
        MediaFailurePayload::DigestMismatch => NodeCapabilityMediaFailure::DigestMismatch,
        MediaFailurePayload::OutputConflict => NodeCapabilityMediaFailure::OutputConflict,
        MediaFailurePayload::StorageFailed => NodeCapabilityMediaFailure::StorageFailed,
        MediaFailurePayload::InspectionFailed => NodeCapabilityMediaFailure::InspectionFailed,
        MediaFailurePayload::FinalizationFailed => NodeCapabilityMediaFailure::FinalizationFailed,
    }
}

fn encode_data_type(value: WorkflowDataType) -> DataTypePayload {
    match value {
        WorkflowDataType::Text => DataTypePayload::Text,
        WorkflowDataType::Image => DataTypePayload::Image,
        WorkflowDataType::Video => DataTypePayload::Video,
        WorkflowDataType::Audio => DataTypePayload::Audio,
    }
}

fn decode_data_type(value: DataTypePayload) -> WorkflowDataType {
    match value {
        DataTypePayload::Text => WorkflowDataType::Text,
        DataTypePayload::Image => WorkflowDataType::Image,
        DataTypePayload::Video => WorkflowDataType::Video,
        DataTypePayload::Audio => WorkflowDataType::Audio,
    }
}
