use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nodes::{
    GenerationProfileAvailabilityObservation, GenerationProfileAvailabilityReaderInterface,
    GenerationProfileAvailabilityRequest, GenerationProfileAvailabilityState,
    GenerationProfileError, GenerationProfileRef,
};

use super::{
    ImageToVideoProviderRouterImpl, TextToImageProviderRouterImpl, TextToSpeechProviderRouterImpl,
};

/// Availability adapter backed only by the three exact router-owned maps.
pub struct ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl {
    text_to_image_router: Arc<TextToImageProviderRouterImpl>,
    image_to_video_router: Arc<ImageToVideoProviderRouterImpl>,
    text_to_speech_router: Arc<TextToSpeechProviderRouterImpl>,
}

impl ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl {
    /// Wires the three exact routers without copying their profile mappings.
    #[must_use]
    pub fn new(
        text_to_image_router: Arc<TextToImageProviderRouterImpl>,
        image_to_video_router: Arc<ImageToVideoProviderRouterImpl>,
        text_to_speech_router: Arc<TextToSpeechProviderRouterImpl>,
    ) -> Self {
        Self { text_to_image_router, image_to_video_router, text_to_speech_router }
    }

    async fn observe(
        &self,
        capability_id: &str,
        profile_ref: &GenerationProfileRef,
    ) -> Result<GenerationProfileAvailabilityState, GenerationProfileError> {
        match capability_id {
            "image.generate_from_text" => Ok(self
                .text_to_image_router
                .observe_generation_profile_availability(profile_ref)
                .await),
            "video.generate_from_image" => Ok(self
                .image_to_video_router
                .observe_generation_profile_availability(profile_ref)
                .await),
            "audio.synthesize_speech_from_text" => Ok(self
                .text_to_speech_router
                .observe_generation_profile_availability(profile_ref)
                .await),
            _ => Err(GenerationProfileError::AvailabilityReadFailed),
        }
    }
}

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface
    for ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl
{
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        let capability_id = request.capability_ref().id().as_str();
        if request.capability_ref().version().major() != 1
            || request.capability_ref().version().minor() != 0
        {
            return Err(GenerationProfileError::AvailabilityReadFailed);
        }
        let mut observations = Vec::with_capacity(request.profile_refs().len());
        for profile_ref in request.profile_refs() {
            ensure_before_deadline(request.deadline())?;
            let state = self.observe(capability_id, profile_ref).await?;
            ensure_before_deadline(request.deadline())?;
            let observed_at_epoch_ms = current_epoch_ms()?;
            observations.push(GenerationProfileAvailabilityObservation::try_new(
                profile_ref.clone(),
                state,
                observed_at_epoch_ms,
                observed_at_epoch_ms + 30_000,
            )?);
        }
        Ok(observations)
    }
}

fn ensure_before_deadline(deadline: Instant) -> Result<(), GenerationProfileError> {
    if Instant::now() >= deadline { Err(GenerationProfileError::DeadlineExceeded) } else { Ok(()) }
}

fn current_epoch_ms() -> Result<i64, GenerationProfileError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GenerationProfileError::AvailabilityReadFailed)?
        .as_millis();
    i64::try_from(millis).map_err(|_| GenerationProfileError::AvailabilityReadFailed)
}
