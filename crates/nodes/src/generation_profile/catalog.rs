use std::collections::{BTreeMap, BTreeSet};

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
};

use super::{
    GenerationProfileDisplayName, GenerationProfileError, GenerationProfileId,
    GenerationProfileRef, GenerationProfileVersion,
};

/// Immutable profile lifecycle including saved-Workflow tombstones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationProfileLifecycleState {
    /// The profile is selectable for new and saved nodes.
    Active,
    /// The profile remains a resolvable tombstone but is not selectable.
    Retired,
}

/// Immutable provider-independent profile definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProfileDefinition {
    profile_ref: GenerationProfileRef,
    display_name: GenerationProfileDisplayName,
    lifecycle_state: GenerationProfileLifecycleState,
    compatible_capabilities: BTreeSet<NodeCapabilityContractRef>,
}

impl GenerationProfileDefinition {
    /// Creates a definition with at least one exact compatible capability.
    pub fn try_new(
        profile_ref: GenerationProfileRef,
        display_name: GenerationProfileDisplayName,
        lifecycle_state: GenerationProfileLifecycleState,
        compatible_capabilities: BTreeSet<NodeCapabilityContractRef>,
    ) -> Result<Self, GenerationProfileError> {
        if compatible_capabilities.is_empty() {
            return Err(GenerationProfileError::InvalidDefinition);
        }
        Ok(Self { profile_ref, display_name, lifecycle_state, compatible_capabilities })
    }
    /// Returns the exact stable identity.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns provider-independent display text.
    #[must_use]
    pub const fn display_name(&self) -> &GenerationProfileDisplayName {
        &self.display_name
    }
    /// Returns Active or Retired lifecycle.
    #[must_use]
    pub const fn lifecycle_state(&self) -> GenerationProfileLifecycleState {
        self.lifecycle_state
    }
    /// Returns exact compatible capability refs.
    #[must_use]
    pub const fn compatible_capabilities(&self) -> &BTreeSet<NodeCapabilityContractRef> {
        &self.compatible_capabilities
    }
}

/// Concrete immutable frozen Generation Profile collection.
pub struct GenerationProfileCatalog {
    definitions: BTreeMap<GenerationProfileRef, GenerationProfileDefinition>,
}

impl GenerationProfileCatalog {
    /// Builds exactly the frozen three-profile MVP catalog.
    pub fn frozen_mvp() -> Result<Self, GenerationProfileError> {
        let definitions = [
            definition(
                "image.high_quality_general",
                "High Quality Image",
                "image.generate_from_text",
            )?,
            definition(
                "video.cinematic_image_animation",
                "Cinematic Image Animation",
                "video.generate_from_image",
            )?,
            definition(
                "speech.multilingual_narration",
                "Multilingual Narration",
                "audio.synthesize_speech_from_text",
            )?,
        ];
        Self::from_definitions(definitions)
    }

    fn from_definitions(
        definitions: impl IntoIterator<Item = GenerationProfileDefinition>,
    ) -> Result<Self, GenerationProfileError> {
        let mut by_ref = BTreeMap::new();
        for definition in definitions {
            if by_ref.insert(definition.profile_ref.clone(), definition).is_some() {
                return Err(GenerationProfileError::InvalidDefinition);
            }
        }
        Ok(Self { definitions: by_ref })
    }

    /// Resolves Active or Retired exact definition without fallback.
    pub fn find_generation_profile(
        &self,
        profile_ref: &GenerationProfileRef,
    ) -> Result<&GenerationProfileDefinition, GenerationProfileError> {
        self.definitions.get(profile_ref).ok_or(GenerationProfileError::ProfileNotFound)
    }

    /// Lists only Active exact-compatible definitions in stable profile-ref order.
    #[must_use]
    pub fn list_active_generation_profiles_for_capability(
        &self,
        capability_ref: &NodeCapabilityContractRef,
    ) -> Vec<&GenerationProfileDefinition> {
        self.definitions
            .values()
            .filter(|definition| {
                definition.lifecycle_state == GenerationProfileLifecycleState::Active
                    && definition.compatible_capabilities.contains(capability_ref)
            })
            .collect()
    }
}

fn definition(
    profile_id: &str,
    display_name: &str,
    capability_id: &str,
) -> Result<GenerationProfileDefinition, GenerationProfileError> {
    GenerationProfileDefinition::try_new(
        GenerationProfileRef::new(
            GenerationProfileId::try_new(profile_id)?,
            GenerationProfileVersion::try_new(1)?,
        ),
        GenerationProfileDisplayName::try_new(display_name)?,
        GenerationProfileLifecycleState::Active,
        BTreeSet::from([NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(capability_id)
                .map_err(|_| GenerationProfileError::InvalidDefinition)?,
            NodeCapabilityContractVersion::new(1, 0)
                .map_err(|_| GenerationProfileError::InvalidDefinition)?,
        )]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_definition_remains_resolvable_but_not_selectable() {
        let mut retired = definition(
            "image.high_quality_general",
            "High Quality Image",
            "image.generate_from_text",
        )
        .unwrap();
        retired.lifecycle_state = GenerationProfileLifecycleState::Retired;
        let profile_ref = retired.profile_ref.clone();
        let capability_ref = retired.compatible_capabilities.iter().next().unwrap().clone();
        let catalog = GenerationProfileCatalog::from_definitions([retired]).unwrap();

        assert_eq!(
            catalog.find_generation_profile(&profile_ref).unwrap().lifecycle_state(),
            GenerationProfileLifecycleState::Retired
        );
        assert!(catalog.list_active_generation_profiles_for_capability(&capability_ref).is_empty());
    }
}
