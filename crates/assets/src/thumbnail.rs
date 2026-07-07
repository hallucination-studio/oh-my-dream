//! Thumbnail generation for stored assets.

use crate::error::{AssetError, Result};
use crate::model::AssetKind;
use image::{ImageBuffer, Rgba};
use std::path::{Path, PathBuf};

const IMAGE_THUMBNAIL_SIZE: u32 = 256;
const VIDEO_THUMBNAIL_WIDTH: u32 = 320;
const VIDEO_THUMBNAIL_HEIGHT: u32 = 180;

pub(crate) fn generate_thumbnail(
    asset_id: &str,
    kind: AssetKind,
    source_path: &Path,
    thumbnails_dir: &Path,
) -> Result<PathBuf> {
    let thumbnail_path = thumbnails_dir.join(format!("{asset_id}.png"));
    match kind {
        AssetKind::Image => generate_image_thumbnail(asset_id, source_path, &thumbnail_path)?,
        AssetKind::Video => generate_video_placeholder(asset_id, &thumbnail_path)?,
    }
    Ok(thumbnail_path)
}

fn generate_image_thumbnail(asset_id: &str, source_path: &Path, output_path: &Path) -> Result<()> {
    let image = image::open(source_path).map_err(|source| AssetError::Thumbnail {
        id: asset_id.to_owned(),
        message: format!("open image `{}`: {source}", source_path.display()),
    })?;
    image.thumbnail(IMAGE_THUMBNAIL_SIZE, IMAGE_THUMBNAIL_SIZE).save(output_path).map_err(
        |source| AssetError::Thumbnail {
            id: asset_id.to_owned(),
            message: format!("write thumbnail `{}`: {source}", output_path.display()),
        },
    )
}

fn generate_video_placeholder(asset_id: &str, output_path: &Path) -> Result<()> {
    // Why a placeholder rather than a real first frame: extracting a frame needs
    // a video decoder (ffmpeg or a bundled codec), which we do not want as a hard
    // dependency in this milestone. A deterministic placeholder keeps the store
    // self-contained; real frame extraction is deferred.
    let image = ImageBuffer::from_fn(VIDEO_THUMBNAIL_WIDTH, VIDEO_THUMBNAIL_HEIGHT, |x, y| {
        if x / 16 == y / 9 { Rgba([90_u8, 120, 150, 255]) } else { Rgba([32_u8, 38, 46, 255]) }
    });
    image.save(output_path).map_err(|source| AssetError::Thumbnail {
        id: asset_id.to_owned(),
        message: format!("write video placeholder `{}`: {source}", output_path.display()),
    })
}
