use async_trait::async_trait;
use nodes::{
    GenerationProfileAvailabilityObservation, GenerationProfileAvailabilityReaderInterface,
    GenerationProfileAvailabilityRequest, GenerationProfileAvailabilityState,
    GenerationProfileError,
};

pub struct GenerationProfileAlwaysAvailableFakeImpl;

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for GenerationProfileAlwaysAvailableFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        request
            .profile_refs()
            .iter()
            .cloned()
            .map(|profile_ref| {
                GenerationProfileAvailabilityObservation::try_new(
                    profile_ref,
                    GenerationProfileAvailabilityState::Available,
                    100,
                    1_000,
                )
            })
            .collect()
    }
}
