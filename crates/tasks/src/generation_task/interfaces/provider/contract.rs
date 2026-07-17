//! Safe Generation Provider identity and route contract projections.

use std::collections::BTreeSet;

use nodes::GenerationProfileRef;

use super::GenerationProviderContractError;
use crate::generation_task::domain::{GenerationProviderId, GenerationProviderRouteId};

const MAX_PROVIDER_DISPLAY_NAME_CHARS: usize = 80;

macro_rules! provider_display_name {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Validates trimmed display text without control characters.
            pub fn try_new(
                value: impl Into<String>,
            ) -> Result<Self, GenerationProviderContractError> {
                let value = value.into();
                let valid = value.trim() == value
                    && (1..=MAX_PROVIDER_DISPLAY_NAME_CHARS).contains(&value.chars().count())
                    && !value.chars().any(char::is_control);
                if !valid {
                    return Err(GenerationProviderContractError::InvalidDisplayName);
                }
                Ok(Self(value))
            }

            /// Returns the exact safe display text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

provider_display_name!(GenerationProviderDisplayName, "Safe user-facing provider display name.");
provider_display_name!(
    GenerationProviderRouteDisplayName,
    "Safe user-facing provider route display name."
);

/// Safe exact route contract exposed to Settings and routing validation.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationProviderRouteContract {
    route_id: GenerationProviderRouteId,
    display_name: GenerationProviderRouteDisplayName,
    compatible_generation_profiles: BTreeSet<GenerationProfileRef>,
}

impl GenerationProviderRouteContract {
    /// Creates a route contract with at least one exact compatible profile.
    pub fn try_new(
        route_id: GenerationProviderRouteId,
        display_name: GenerationProviderRouteDisplayName,
        compatible_generation_profiles: BTreeSet<GenerationProfileRef>,
    ) -> Result<Self, GenerationProviderContractError> {
        if compatible_generation_profiles.is_empty() {
            return Err(GenerationProviderContractError::EmptyCompatibleProfiles);
        }
        Ok(Self { route_id, display_name, compatible_generation_profiles })
    }

    /// Returns the stable route identity.
    #[must_use]
    pub const fn route_id(&self) -> &GenerationProviderRouteId {
        &self.route_id
    }

    /// Returns the safe route display name.
    #[must_use]
    pub const fn display_name(&self) -> &GenerationProviderRouteDisplayName {
        &self.display_name
    }

    /// Returns the non-empty exact compatible profile set.
    #[must_use]
    pub const fn compatible_generation_profiles(&self) -> &BTreeSet<GenerationProfileRef> {
        &self.compatible_generation_profiles
    }
}

macro_rules! focused_contract {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(Vec<GenerationProviderRouteContract>);

        impl $name {
            /// Builds a non-empty route set with unique identities.
            pub fn try_new(
                mut routes: Vec<GenerationProviderRouteContract>,
            ) -> Result<Self, GenerationProviderContractError> {
                if routes.is_empty() {
                    return Err(GenerationProviderContractError::EmptyRoutes);
                }
                routes.sort_by(|left, right| left.route_id().cmp(right.route_id()));
                if routes.windows(2).any(|pair| pair[0].route_id() == pair[1].route_id()) {
                    return Err(GenerationProviderContractError::DuplicateRouteId);
                }
                Ok(Self(routes))
            }

            /// Returns routes in stable route-ID order.
            #[must_use]
            pub fn routes(&self) -> &[GenerationProviderRouteContract] {
                &self.0
            }

            /// Resolves one exact declared route contract without fallback.
            #[must_use]
            pub fn find_route(
                &self,
                route_id: &GenerationProviderRouteId,
            ) -> Option<&GenerationProviderRouteContract> {
                self.0
                    .binary_search_by(|route| route.route_id().cmp(route_id))
                    .ok()
                    .map(|index| &self.0[index])
            }
        }
    };
}

focused_contract!(TextGenerationProviderContract, "Complete Text provider route contract.");
focused_contract!(ImageGenerationProviderContract, "Complete Image provider route contract.");
focused_contract!(VideoGenerationProviderContract, "Complete Video provider route contract.");
focused_contract!(VoiceGenerationProviderContract, "Complete Voice provider route contract.");

/// Safe provider contract mechanically derived from one provider implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderContract {
    provider_id: GenerationProviderId,
    display_name: GenerationProviderDisplayName,
    text: Option<TextGenerationProviderContract>,
    image: Option<ImageGenerationProviderContract>,
    video: Option<VideoGenerationProviderContract>,
    voice: Option<VoiceGenerationProviderContract>,
}

impl GenerationProviderContract {
    pub(super) fn new(
        provider_id: GenerationProviderId,
        display_name: GenerationProviderDisplayName,
        text: Option<TextGenerationProviderContract>,
        image: Option<ImageGenerationProviderContract>,
        video: Option<VideoGenerationProviderContract>,
        voice: Option<VoiceGenerationProviderContract>,
    ) -> Self {
        Self { provider_id, display_name, text, image, video, voice }
    }

    /// Returns the stable provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &GenerationProviderId {
        &self.provider_id
    }

    /// Returns the safe provider display name.
    #[must_use]
    pub const fn display_name(&self) -> &GenerationProviderDisplayName {
        &self.display_name
    }

    /// Returns the complete Text contract when contributed.
    #[must_use]
    pub const fn text(&self) -> Option<&TextGenerationProviderContract> {
        self.text.as_ref()
    }

    /// Returns the complete Image contract when contributed.
    #[must_use]
    pub const fn image(&self) -> Option<&ImageGenerationProviderContract> {
        self.image.as_ref()
    }

    /// Returns the complete Video contract when contributed.
    #[must_use]
    pub const fn video(&self) -> Option<&VideoGenerationProviderContract> {
        self.video.as_ref()
    }

    /// Returns the complete Voice contract when contributed.
    #[must_use]
    pub const fn voice(&self) -> Option<&VoiceGenerationProviderContract> {
        self.voice.as_ref()
    }
}
