//! Concrete media-inspection, time, and identity adapters for Asset interfaces.

mod ffprobe;

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use assets::asset::application::{
    AssetApplicationError, AssetImportSourceLease, AssetInspectedMedia,
};
use assets::asset::domain::{
    AssetContentFinalizationId, AssetCreatedAt, AssetId, AssetImportId, AssetMediaFacts,
    AssetMediaKind, AssetMediaMimeType, AssetPreviewLeaseId,
};
use assets::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetMediaInspectorInterface,
};
use async_trait::async_trait;
use image::GenericImageView;
use tokio::io::AsyncReadExt;

/// Rust-image and bundled-ffprobe implementation of media inspection.
pub struct ImageAndFfprobeAssetMediaInspectorAdapterImpl {
    ffprobe_path: PathBuf,
}

impl ImageAndFfprobeAssetMediaInspectorAdapterImpl {
    /// Selects the bundled private ffprobe executable.
    #[must_use]
    pub fn new(ffprobe_path: PathBuf) -> Self {
        Self { ffprobe_path }
    }
}

#[async_trait]
impl AssetMediaInspectorInterface for ImageAndFfprobeAssetMediaInspectorAdapterImpl {
    async fn inspect_asset_media(
        &self,
        source: AssetImportSourceLease,
        expected_media_kind: AssetMediaKind,
    ) -> Result<AssetInspectedMedia, AssetApplicationError> {
        let deadline = source.deadline();
        let stream = source.try_take_stream()?;
        let bytes = read_bounded(stream, maximum_bytes(expected_media_kind), deadline).await?;
        match expected_media_kind {
            AssetMediaKind::Image => inspect_image(&bytes),
            AssetMediaKind::Video | AssetMediaKind::Audio => {
                ffprobe::inspect(&self.ffprobe_path, bytes, expected_media_kind, deadline).await
            }
        }
    }
}

/// UTC system clock implementation of the Asset clock boundary.
pub struct SystemAssetClockAdapterImpl;

impl AssetClockInterface for SystemAssetClockAdapterImpl {
    fn current_asset_time(&self) -> Result<AssetCreatedAt, AssetApplicationError> {
        let milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AssetApplicationError::IdentityConflict)?
            .as_millis();
        let milliseconds =
            i64::try_from(milliseconds).map_err(|_| AssetApplicationError::IdentityConflict)?;
        AssetCreatedAt::from_utc_milliseconds(milliseconds)
            .map_err(|_| AssetApplicationError::IdentityConflict)
    }
}

/// Operating-system-random UUIDv4 implementation of all Asset identity methods.
pub struct UuidV4AssetIdentityGeneratorAdapterImpl;

impl AssetIdentityGeneratorInterface for UuidV4AssetIdentityGeneratorAdapterImpl {
    fn generate_asset_id(&self) -> Result<AssetId, AssetApplicationError> {
        AssetId::from_uuid(uuid::Uuid::new_v4()).map_err(|_| identity_failure())
    }

    fn generate_asset_import_id(&self) -> Result<AssetImportId, AssetApplicationError> {
        AssetImportId::from_uuid(uuid::Uuid::new_v4()).map_err(|_| identity_failure())
    }

    fn generate_asset_content_finalization_id(
        &self,
    ) -> Result<AssetContentFinalizationId, AssetApplicationError> {
        AssetContentFinalizationId::from_uuid(uuid::Uuid::new_v4()).map_err(|_| identity_failure())
    }

    fn generate_asset_preview_lease_id(
        &self,
    ) -> Result<AssetPreviewLeaseId, AssetApplicationError> {
        AssetPreviewLeaseId::from_uuid(uuid::Uuid::new_v4()).map_err(|_| identity_failure())
    }
}

async fn read_bounded(
    stream: std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>>,
    maximum: u64,
    deadline: Instant,
) -> Result<Vec<u8>, AssetApplicationError> {
    let mut bytes = Vec::new();
    let mut bounded = stream.take(maximum + 1);
    tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        bounded.read_to_end(&mut bytes),
    )
    .await
    .map_err(|_| AssetApplicationError::DeadlineExceeded)?
    .map_err(|_| AssetApplicationError::InspectionFailed)?;
    if u64::try_from(bytes.len()).map_err(|_| AssetApplicationError::MediaSizeLimitExceeded)?
        > maximum
    {
        return Err(AssetApplicationError::MediaSizeLimitExceeded);
    }
    Ok(bytes)
}

fn inspect_image(bytes: &[u8]) -> Result<AssetInspectedMedia, AssetApplicationError> {
    let (format, mime) = sniff_image(bytes)?;
    let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| AssetApplicationError::InvalidMedia)?;
    let facts = AssetMediaFacts::try_image(width, height)
        .map_err(|_| AssetApplicationError::InvalidMedia)?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|_| AssetApplicationError::InvalidMedia)?;
    if image.dimensions() != (width, height) {
        return Err(AssetApplicationError::InvalidMedia);
    }
    AssetInspectedMedia::try_new(mime, facts)
}

fn sniff_image(
    bytes: &[u8],
) -> Result<(image::ImageFormat, AssetMediaMimeType), AssetApplicationError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if png_has_chunk(bytes, b"acTL") {
            return Err(AssetApplicationError::InvalidMedia);
        }
        return Ok((image::ImageFormat::Png, AssetMediaMimeType::ImagePng));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok((image::ImageFormat::Jpeg, AssetMediaMimeType::ImageJpeg));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        if webp_is_animated(bytes) {
            return Err(AssetApplicationError::InvalidMedia);
        }
        return Ok((image::ImageFormat::WebP, AssetMediaMimeType::ImageWebp));
    }
    Err(AssetApplicationError::InvalidMedia)
}

fn webp_is_animated(bytes: &[u8]) -> bool {
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let chunk = &bytes[offset..offset + 4];
        let Ok(size_bytes) = bytes[offset + 4..offset + 8].try_into() else { return true };
        let Ok(size) = usize::try_from(u32::from_le_bytes(size_bytes)) else { return true };
        if chunk == b"ANIM"
            || (chunk == b"VP8X" && bytes.get(offset + 8).is_some_and(|value| value & 2 != 0))
        {
            return true;
        }
        let Some(next) = offset
            .checked_add(8)
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(size & 1))
        else {
            return true;
        };
        if next <= offset || next > bytes.len() {
            return false;
        }
        offset = next;
    }
    false
}

fn png_has_chunk(bytes: &[u8], expected: &[u8; 4]) -> bool {
    let mut offset = 8;
    while offset + 12 <= bytes.len() {
        let Ok(length_bytes) = bytes[offset..offset + 4].try_into() else { return false };
        let Ok(length) = usize::try_from(u32::from_be_bytes(length_bytes)) else { return false };
        if bytes.get(offset + 4..offset + 8) == Some(expected) {
            return true;
        }
        let Some(next) = offset.checked_add(12).and_then(|value| value.checked_add(length)) else {
            return false;
        };
        if next <= offset || next > bytes.len() {
            return false;
        }
        offset = next;
    }
    false
}

const fn maximum_bytes(kind: AssetMediaKind) -> u64 {
    match kind {
        AssetMediaKind::Image => 32 * 1024 * 1024,
        AssetMediaKind::Video => 512 * 1024 * 1024,
        AssetMediaKind::Audio => 64 * 1024 * 1024,
    }
}

const fn identity_failure() -> AssetApplicationError {
    AssetApplicationError::IdentityConflict
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_image_inspector_returns_png_dimensions_and_rejects_animation_marker() {
        let image = image::DynamicImage::new_rgb8(3, 2);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let inspected = inspect_image(bytes.get_ref()).unwrap();
        assert_eq!(inspected.mime_type(), AssetMediaMimeType::ImagePng);
        let AssetMediaFacts::Image(facts) = inspected.media_facts() else { panic!("image facts") };
        assert_eq!((facts.width(), facts.height()), (3, 2));

        let mut animated = bytes.into_inner();
        animated.splice(8..8, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
        assert_eq!(inspect_image(&animated), Err(AssetApplicationError::InvalidMedia));
    }

    #[test]
    fn system_clock_and_all_identity_methods_return_valid_values() {
        assert!(
            SystemAssetClockAdapterImpl.current_asset_time().unwrap().as_utc_milliseconds() > 0
        );
        let generator = UuidV4AssetIdentityGeneratorAdapterImpl;
        let values = [
            generator.generate_asset_id().unwrap().as_uuid(),
            generator.generate_asset_import_id().unwrap().as_uuid(),
            generator.generate_asset_content_finalization_id().unwrap().as_uuid(),
            generator.generate_asset_preview_lease_id().unwrap().as_uuid(),
        ];
        assert!(values.iter().all(|value| value.get_version() == Some(uuid::Version::Random)));
        let unique = values.into_iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 4);
    }
}
