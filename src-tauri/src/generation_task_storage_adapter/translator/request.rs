use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use serde::{Deserialize, Serialize};
use tasks::generation_task::domain::*;
use uuid::Uuid;

use super::array;

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum RequestDto {
    Text {
        prompt: String,
    },
    Image {
        prompt: String,
        aspect_ratio: String,
    },
    Voice {
        text: String,
    },
    Video {
        input_asset_id: String,
        input_media_kind: String,
        input_content_hash: Vec<u8>,
        duration_seconds: u8,
        prompt: Option<String>,
    },
}

pub(super) fn encode(request: &GenerationTaskRequest) -> Result<String, ()> {
    serde_json::to_string(&RequestDto::from_domain(request)).map_err(|_| ())
}

pub(super) fn decode(encoded: &str) -> Result<GenerationTaskRequest, ()> {
    let dto: RequestDto = serde_json::from_str(encoded).map_err(|_| ())?;
    if serde_json::to_string(&dto).map_err(|_| ())? != encoded {
        return Err(());
    }
    Ok(match dto {
        RequestDto::Text { prompt } => {
            GenerationTaskRequest::Text(TextGenerationSpec::new(text(prompt)?))
        }
        RequestDto::Image { prompt, aspect_ratio } => GenerationTaskRequest::Image(
            ImageGenerationSpec::new(text(prompt)?, parse_aspect_ratio(&aspect_ratio)?),
        ),
        RequestDto::Voice { text: value } => {
            GenerationTaskRequest::Voice(VoiceGenerationSpec::new(text(value)?))
        }
        RequestDto::Video {
            input_asset_id,
            input_media_kind,
            input_content_hash,
            duration_seconds,
            prompt,
        } => GenerationTaskRequest::Video(
            VideoGenerationSpec::try_new(
                AssetSnapshotRef::new(
                    AssetId::from_uuid(Uuid::parse_str(&input_asset_id).map_err(|_| ())?)
                        .map_err(|_| ())?,
                    parse_media_kind(&input_media_kind)?,
                    AssetContentDigest::from_bytes(array::<32>(&input_content_hash)?),
                ),
                match duration_seconds {
                    5 => VideoDurationSeconds::Five,
                    10 => VideoDurationSeconds::Ten,
                    _ => return Err(()),
                },
                prompt.map(text).transpose()?,
            )
            .map_err(|_| ())?,
        ),
    })
}

impl RequestDto {
    fn from_domain(request: &GenerationTaskRequest) -> Self {
        match request {
            GenerationTaskRequest::Text(spec) => {
                Self::Text { prompt: spec.prompt().as_str().into() }
            }
            GenerationTaskRequest::Image(spec) => Self::Image {
                prompt: spec.prompt().as_str().into(),
                aspect_ratio: aspect_ratio(spec.aspect_ratio()).into(),
            },
            GenerationTaskRequest::Voice(spec) => Self::Voice { text: spec.text().as_str().into() },
            GenerationTaskRequest::Video(spec) => Self::Video {
                input_asset_id: spec.input_image().asset_id().to_string(),
                input_media_kind: media_kind(spec.input_image().media_kind()).into(),
                input_content_hash: spec.input_image().content_hash().as_bytes().to_vec(),
                duration_seconds: spec.duration_seconds().get(),
                prompt: spec.prompt().map(|value| value.as_str().into()),
            },
        }
    }
}

fn text(value: String) -> Result<GenerationTaskText, ()> {
    GenerationTaskText::try_new(value).map_err(|_| ())
}

fn aspect_ratio(value: ImageAspectRatio) -> &'static str {
    match value {
        ImageAspectRatio::Square => "Square",
        ImageAspectRatio::Landscape4To3 => "Landscape4To3",
        ImageAspectRatio::Portrait3To4 => "Portrait3To4",
        ImageAspectRatio::Landscape16To9 => "Landscape16To9",
        ImageAspectRatio::Portrait9To16 => "Portrait9To16",
    }
}

fn parse_aspect_ratio(value: &str) -> Result<ImageAspectRatio, ()> {
    match value {
        "Square" => Ok(ImageAspectRatio::Square),
        "Landscape4To3" => Ok(ImageAspectRatio::Landscape4To3),
        "Portrait3To4" => Ok(ImageAspectRatio::Portrait3To4),
        "Landscape16To9" => Ok(ImageAspectRatio::Landscape16To9),
        "Portrait9To16" => Ok(ImageAspectRatio::Portrait9To16),
        _ => Err(()),
    }
}

fn media_kind(value: AssetMediaKind) -> &'static str {
    match value {
        AssetMediaKind::Image => "Image",
        AssetMediaKind::Video => "Video",
        AssetMediaKind::Audio => "Audio",
    }
}

fn parse_media_kind(value: &str) -> Result<AssetMediaKind, ()> {
    match value {
        "Image" => Ok(AssetMediaKind::Image),
        "Video" => Ok(AssetMediaKind::Video),
        "Audio" => Ok(AssetMediaKind::Audio),
        _ => Err(()),
    }
}
