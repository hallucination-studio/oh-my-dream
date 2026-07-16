use engine::node_capability::{WorkflowTextPart, WorkflowTextValue};
use nodes::{ImageAspectRatio, NodeCapabilityDeclaredMediaFacts};
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(super) struct FalTextToImageRequestDto<'a> {
    pub prompt: String,
    pub aspect_ratio: &'a str,
    pub num_images: u8,
    pub output_format: &'a str,
    pub safety_tolerance: &'a str,
    pub enhance_prompt: bool,
}

#[derive(Deserialize)]
pub(super) struct FalSubmitResponseDto {
    pub request_id: String,
}

#[derive(Deserialize)]
pub(super) struct FalStatusResponseDto {
    pub status: String,
}

#[derive(Deserialize)]
pub(super) struct FalTextToImageResultDto {
    pub images: Vec<FalImageDto>,
    pub has_nsfw_concepts: Vec<bool>,
}

#[derive(Deserialize)]
pub(super) struct FalImageDto {
    pub url: String,
    pub content_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub(super) struct FalImageResult {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

pub(super) fn literal_text(value: &WorkflowTextValue) -> Result<String, ()> {
    let mut text = String::new();
    for part in value.parts() {
        match part {
            WorkflowTextPart::Literal(value) => text.push_str(value),
            WorkflowTextPart::InputItemReference(_) => return Err(()),
        }
    }
    if text.is_empty() { Err(()) } else { Ok(text) }
}

pub(super) fn aspect_ratio(value: ImageAspectRatio) -> &'static str {
    match value {
        ImageAspectRatio::Square => "1:1",
        ImageAspectRatio::LandscapeFourByThree => "4:3",
        ImageAspectRatio::PortraitThreeByFour => "3:4",
        ImageAspectRatio::LandscapeSixteenByNine => "16:9",
        ImageAspectRatio::PortraitNineBySixteen => "9:16",
    }
}

pub(super) fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn validated_result(result: FalTextToImageResultDto) -> Result<FalImageResult, ()> {
    let [image] = result.images.as_slice() else {
        return Err(());
    };
    if !valid_media_url(&image.url)
        || image.content_type != "image/png"
        || result.has_nsfw_concepts.iter().any(|value| *value)
    {
        return Err(());
    }
    let width = image.width.ok_or(())?;
    let height = image.height.ok_or(())?;
    NodeCapabilityDeclaredMediaFacts::try_image(width, height).map_err(|_| ())?;
    Ok(FalImageResult { url: image.url.clone(), width, height })
}

fn valid_media_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    url.scheme() == "https"
        && url.port().is_none()
        && (host == "fal.media" || host.ends_with(".fal.media"))
}
