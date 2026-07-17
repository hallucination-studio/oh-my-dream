use std::sync::Arc;

use engine::node_capability::{
    WorkflowDataType, WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeCapabilityInterface,
};
use nodes::*;
use projects::project::domain::ProjectId;

pub fn configured_asset_capability(
    kind: WorkflowDataType,
    project_id: ProjectId,
    asset_id: WorkflowManagedAssetIdBoundaryValue,
    fingerprint: WorkflowManagedContentFingerprint,
    bytes: Vec<u8>,
) -> Arc<dyn WorkflowNodeCapabilityInterface> {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    match kind {
        WorkflowDataType::Image => {
            let reference = WorkflowManagedImageRef::new(asset_id, fingerprint);
            reader
                .register_managed_media(
                    project_id,
                    NodeCapabilityManagedMediaReference::Image(reference),
                    NodeCapabilityMediaMimeType::ImagePng,
                    NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
                    bytes,
                )
                .unwrap();
            Arc::new(ReadImageAssetCapabilityImpl::try_new(reader).unwrap())
        }
        WorkflowDataType::Video => {
            let reference = WorkflowManagedVideoRef::new(asset_id, fingerprint);
            reader
                .register_managed_media(
                    project_id,
                    NodeCapabilityManagedMediaReference::Video(reference),
                    NodeCapabilityMediaMimeType::VideoMp4,
                    NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 1_000, false).unwrap(),
                    bytes,
                )
                .unwrap();
            Arc::new(ReadVideoAssetCapabilityImpl::try_new(reader).unwrap())
        }
        WorkflowDataType::Audio => {
            let reference = WorkflowManagedAudioRef::new(asset_id, fingerprint);
            reader
                .register_managed_media(
                    project_id,
                    NodeCapabilityManagedMediaReference::Audio(reference),
                    NodeCapabilityMediaMimeType::AudioMpeg,
                    NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 2).unwrap(),
                    bytes,
                )
                .unwrap();
            Arc::new(ReadAudioAssetCapabilityImpl::try_new(reader).unwrap())
        }
        WorkflowDataType::Text => unreachable!(),
    }
}
