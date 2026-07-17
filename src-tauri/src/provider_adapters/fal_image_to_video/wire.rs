use engine::node_capability::{WorkflowTextPart, WorkflowTextValue};
use nodes::ImageToVideoDurationSeconds;
use reqwest::Url;
use serde::{Deserialize, Serialize};

pub(super) const DEFAULT_PROMPT: &str = "Animate the source image with coherent natural motion.";

#[derive(Serialize)]
pub(super) struct FalImageToVideoRequestDto {
    pub start_image_url: String,
    pub prompt: String,
    pub duration: &'static str,
    pub generate_audio: bool,
}

#[derive(Deserialize)]
pub(super) struct FalVideoResultDto {
    pub video: FalVideoDto,
}

#[derive(Deserialize)]
pub(super) struct FalVideoDto {
    pub url: String,
    pub content_type: String,
    pub file_size: Option<u64>,
}

pub(super) fn prompt(value: Option<&WorkflowTextValue>) -> Result<String, ()> {
    let Some(value) = value else {
        return Ok(DEFAULT_PROMPT.to_owned());
    };
    let mut text = String::new();
    for part in value.parts() {
        match part {
            WorkflowTextPart::Literal(value) => text.push_str(value),
            WorkflowTextPart::InputItemReference(_) => return Err(()),
        }
    }
    if text.is_empty() { Err(()) } else { Ok(text) }
}

pub(super) const fn duration(value: ImageToVideoDurationSeconds) -> &'static str {
    match value {
        ImageToVideoDurationSeconds::Five => "5",
        ImageToVideoDurationSeconds::Ten => "10",
    }
}

pub(super) fn validate_result(value: FalVideoResultDto) -> Result<FalVideoDto, ()> {
    let host = Url::parse(&value.video.url)
        .ok()
        .filter(|url| url.scheme() == "https" && url.port().is_none())
        .and_then(|url| url.host_str().map(str::to_owned))
        .ok_or(())?;
    if value.video.content_type != "video/mp4"
        || !(host == "fal.media"
            || host.ends_with(".fal.media")
            || host == "storage.googleapis.com")
        || value.video.file_size == Some(0)
        || value.video.file_size.is_some_and(|size| size > 512 * 1024 * 1024)
    {
        return Err(());
    }
    Ok(value.video)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_media_contract_and_out_of_bounds_size() {
        let valid = || FalVideoResultDto {
            video: FalVideoDto {
                url: "https://v3b.fal.media/files/video.mp4".into(),
                content_type: "video/mp4".into(),
                file_size: Some(16),
            },
        };
        assert!(validate_result(valid()).is_ok());

        let mut wrong_mime = valid();
        wrong_mime.video.content_type = "application/json".into();
        assert!(validate_result(wrong_mime).is_err());

        let mut private_host = valid();
        private_host.video.url = "https://127.0.0.1/video.mp4".into();
        assert!(validate_result(private_host).is_err());

        let mut too_large = valid();
        too_large.video.file_size = Some(512 * 1024 * 1024 + 1);
        assert!(validate_result(too_large).is_err());
    }
}
