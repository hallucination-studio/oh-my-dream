use assets::asset::{application::AssetApplicationError, domain::AssetMediaKind};
use engine::node_capability::{NodeCapabilityMediaFailure, WorkflowDataType};
use nodes::NodeCapabilityMediaBoundaryError;
use nodes::NodeCapabilityMediaKind;

pub(super) fn invalid_media() -> NodeCapabilityMediaBoundaryError {
    NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::InvalidMedia)
}

pub(super) const fn media_kind(value: NodeCapabilityMediaKind) -> AssetMediaKind {
    match value {
        NodeCapabilityMediaKind::Image => AssetMediaKind::Image,
        NodeCapabilityMediaKind::Video => AssetMediaKind::Video,
        NodeCapabilityMediaKind::Audio => AssetMediaKind::Audio,
    }
}

pub(super) fn map_asset_error(error: AssetApplicationError) -> NodeCapabilityMediaBoundaryError {
    use AssetApplicationError as Error;
    let media = match error {
        Error::Cancelled => return NodeCapabilityMediaBoundaryError::Cancelled,
        Error::DeadlineExceeded => return NodeCapabilityMediaBoundaryError::DeadlineExceeded,
        Error::NotFound | Error::NotVisible | Error::ContentPending | Error::ContentMissing => {
            NodeCapabilityMediaFailure::Unavailable
        }
        Error::MediaKindMismatch { expected, observed } => {
            NodeCapabilityMediaFailure::KindMismatch {
                expected: workflow_type(expected),
                observed: workflow_type(observed),
            }
        }
        Error::InvalidMedia => NodeCapabilityMediaFailure::InvalidMedia,
        Error::MediaSizeLimitExceeded => NodeCapabilityMediaFailure::SizeLimitExceeded,
        Error::ContentDigestMismatch => NodeCapabilityMediaFailure::DigestMismatch,
        Error::NodeOutputConflict | Error::IdentityConflict => {
            NodeCapabilityMediaFailure::OutputConflict
        }
        Error::ManagedStorageFailed => NodeCapabilityMediaFailure::StorageFailed,
        Error::InspectionFailed => NodeCapabilityMediaFailure::InspectionFailed,
        Error::FinalizationFailed => NodeCapabilityMediaFailure::FinalizationFailed,
        Error::PreviewLeaseInvalid | Error::PreviewLeaseExpired | Error::PreviewRangeInvalid => {
            NodeCapabilityMediaFailure::Unavailable
        }
    };
    NodeCapabilityMediaBoundaryError::Media(media)
}

const fn workflow_type(value: AssetMediaKind) -> WorkflowDataType {
    match value {
        AssetMediaKind::Image => WorkflowDataType::Image,
        AssetMediaKind::Video => WorkflowDataType::Video,
        AssetMediaKind::Audio => WorkflowDataType::Audio,
    }
}
