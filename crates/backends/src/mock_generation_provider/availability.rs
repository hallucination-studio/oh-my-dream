use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use engine::node_capability::NodeCapabilityContractRef;
use nodes::{
    GenerationProfileAvailabilityObservation, GenerationProfileAvailabilityReaderInterface,
    GenerationProfileAvailabilityRequest, GenerationProfileAvailabilityState,
    GenerationProfileError, GenerationProfileUnavailableReason,
};
use tasks::generation_task::GenerationTaskRequestKind;

use super::MockGenerationProviderRegistryImpl;

/// Bulk profile availability backed by the active immutable Mock registry map.
pub struct GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl {
    registry: Arc<MockGenerationProviderRegistryImpl>,
}

impl GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl {
    /// Uses the same registry that resolves new Task targets and recovery bindings.
    #[must_use]
    pub const fn new(registry: Arc<MockGenerationProviderRegistryImpl>) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl GenerationProfileAvailabilityReaderInterface
    for GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl
{
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        ensure_before_deadline(request.deadline())?;
        let request_kind = capability_request_kind(request.capability_ref())?;
        let observed_at = current_epoch_milliseconds()?;
        request
            .profile_refs()
            .iter()
            .map(|profile_ref| {
                ensure_before_deadline(request.deadline())?;
                let state = if self.registry.target_for_profile(profile_ref, request_kind).is_ok() {
                    GenerationProfileAvailabilityState::Available
                } else {
                    GenerationProfileAvailabilityState::Unavailable {
                        reason: GenerationProfileUnavailableReason::NoConfiguredRoute,
                        retry_after: None,
                    }
                };
                GenerationProfileAvailabilityObservation::try_new(
                    profile_ref.clone(),
                    state,
                    observed_at,
                    observed_at + 30_000,
                )
            })
            .collect()
    }
}

fn capability_request_kind(
    capability_ref: &NodeCapabilityContractRef,
) -> Result<GenerationTaskRequestKind, GenerationProfileError> {
    if capability_ref.version().major() != 1 || capability_ref.version().minor() != 0 {
        return Err(GenerationProfileError::AvailabilityReadFailed);
    }
    match capability_ref.id().as_str() {
        "image.generate_from_text" => Ok(GenerationTaskRequestKind::Image),
        "video.generate_from_image" => Ok(GenerationTaskRequestKind::Video),
        "audio.synthesize_speech_from_text" => Ok(GenerationTaskRequestKind::Voice),
        _ => Err(GenerationProfileError::AvailabilityReadFailed),
    }
}

fn ensure_before_deadline(deadline: Instant) -> Result<(), GenerationProfileError> {
    if Instant::now() < deadline { Ok(()) } else { Err(GenerationProfileError::DeadlineExceeded) }
}

fn current_epoch_milliseconds() -> Result<i64, GenerationProfileError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GenerationProfileError::AvailabilityReadFailed)?
        .as_millis();
    i64::try_from(milliseconds).map_err(|_| GenerationProfileError::AvailabilityReadFailed)
}
