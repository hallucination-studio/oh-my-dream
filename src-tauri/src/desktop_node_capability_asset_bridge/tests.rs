use assets::asset::domain::{AssetMediaFacts, AssetMediaMimeType};
use nodes::{
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaKind, NodeCapabilityMediaMimeType,
};

use super::{declared_facts, mime_type};

#[test]
fn inspected_asset_media_metadata_is_translated_without_reinterpretation() {
    let image = declared_facts(AssetMediaFacts::try_image(640, 480).unwrap()).unwrap();
    let video =
        declared_facts(AssetMediaFacts::try_video(1920, 1080, 1_500, true).unwrap()).unwrap();
    let audio = declared_facts(AssetMediaFacts::try_audio(900, 48_000, 2).unwrap()).unwrap();

    assert!(matches!(image, NodeCapabilityDeclaredMediaFacts::Image(_)));
    assert!(matches!(video, NodeCapabilityDeclaredMediaFacts::Video(_)));
    assert!(matches!(audio, NodeCapabilityDeclaredMediaFacts::Audio(_)));
    assert_eq!(mime_type(AssetMediaMimeType::ImagePng), NodeCapabilityMediaMimeType::ImagePng);
    assert_eq!(
        mime_type(AssetMediaMimeType::VideoMp4).media_kind(),
        NodeCapabilityMediaKind::Video
    );
    assert_eq!(
        mime_type(AssetMediaMimeType::AudioMpeg).media_kind(),
        NodeCapabilityMediaKind::Audio
    );
}
