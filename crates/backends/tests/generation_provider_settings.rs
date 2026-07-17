use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backends::{
    generation_provider_settings::*, mock_generation_provider::MockGenerationProviderAdapterImpl,
};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use tasks::generation_task::*;

#[tokio::test]
async fn get_projects_exact_ordered_mock_choices_and_sanitized_bindings() {
    let repository = Arc::new(SettingsRepositoryFakeImpl::default());
    let get = GenerationProviderSettingsGetUseCase::new(repository, mock_contracts());

    let view = get.get_generation_provider_settings().await.unwrap();

    assert_eq!(view.settings_revision.get(), 1);
    assert_eq!(view.profiles.len(), 3);
    assert_eq!(view.profiles[0].profile_ref.to_string(), "image.high_quality_general@1");
    assert_eq!(view.profiles[0].generation_kind, GenerationTaskRequestKind::Image);
    assert_eq!(view.profiles[0].provider_choices.len(), 1);
    assert_eq!(view.profiles[0].provider_choices[0].provider_id.as_str(), "mock");
    assert_eq!(view.profiles[0].provider_choices[0].display_name, "Mock");
    assert_eq!(
        view.profiles[0].provider_choices[0].routes[0].route_id.as_str(),
        "mock.image.high-quality-general.v1"
    );
    assert!(view.profiles.iter().all(|profile| profile.selected_binding.is_some()));
}

#[tokio::test]
async fn apply_distinguishes_commit_noop_conflict_and_invalid_tuple() {
    let repository = Arc::new(SettingsRepositoryFakeImpl::default());
    let apply = GenerationProviderSettingsApplyUseCase::new(repository.clone(), mock_contracts());
    let image = image_binding();

    let unchanged = apply
        .apply_generation_provider_settings(
            revision(1),
            GenerationProviderSettingsMutation::SetBinding(image.clone()),
        )
        .await
        .unwrap();
    assert_eq!(unchanged.settings_revision, revision(1));

    let removed = apply
        .apply_generation_provider_settings(
            revision(1),
            GenerationProviderSettingsMutation::RemoveBinding {
                profile_ref: image.profile_ref().clone(),
                generation_kind: image.generation_kind(),
            },
        )
        .await
        .unwrap();
    assert_eq!(removed.settings_revision, revision(2));
    assert!(removed.profiles[0].selected_binding.is_none());

    assert_eq!(
        apply
            .apply_generation_provider_settings(
                revision(1),
                GenerationProviderSettingsMutation::SetBinding(image.clone()),
            )
            .await,
        Err(GenerationProviderSettingsError::RevisionConflict)
    );
    let invalid = GenerationProviderSettingsBinding::new(
        image.profile_ref().clone(),
        image.generation_kind(),
        GenerationProviderId::try_new("other").unwrap(),
        image.route_id().clone(),
    );
    assert_eq!(
        apply
            .apply_generation_provider_settings(
                revision(2),
                GenerationProviderSettingsMutation::SetBinding(invalid),
            )
            .await,
        Err(GenerationProviderSettingsError::InvalidMutation)
    );
}

#[tokio::test]
async fn get_rejects_duplicate_provider_contracts() {
    let repository = Arc::new(SettingsRepositoryFakeImpl::default());
    let contract = mock_contracts()[0].clone();
    let get = GenerationProviderSettingsGetUseCase::new(
        repository,
        Arc::new(vec![contract.clone(), contract]),
    );

    assert_eq!(
        get.get_generation_provider_settings().await,
        Err(GenerationProviderSettingsError::ContractProjection)
    );
}

struct SettingsRepositoryFakeImpl {
    snapshot: Mutex<GenerationProviderSettingsSnapshot>,
}

impl Default for SettingsRepositoryFakeImpl {
    fn default() -> Self {
        Self { snapshot: Mutex::new(default_snapshot()) }
    }
}

#[async_trait]
impl GenerationProviderSettingsRepositoryInterface for SettingsRepositoryFakeImpl {
    async fn load_generation_provider_settings_snapshot(
        &self,
    ) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    async fn apply_generation_provider_settings_mutation(
        &self,
        expected_revision: GenerationProviderSettingsRevision,
        mutation: GenerationProviderSettingsMutation,
    ) -> Result<GenerationProviderSettingsMutationResult, GenerationProviderSettingsError> {
        let mut current = self.snapshot.lock().unwrap();
        if current.revision() != expected_revision {
            return Ok(GenerationProviderSettingsMutationResult::RevisionConflict);
        }
        let mut bindings = current.bindings().to_vec();
        let index = bindings.iter().position(|binding| mutation_matches(&mutation, binding));
        let changed = match mutation {
            GenerationProviderSettingsMutation::SetBinding(binding) => match index {
                Some(index) if bindings[index] == binding => false,
                Some(index) => {
                    bindings[index] = binding;
                    true
                }
                None => {
                    bindings.push(binding);
                    true
                }
            },
            GenerationProviderSettingsMutation::RemoveBinding { .. } => match index {
                Some(index) => {
                    bindings.remove(index);
                    true
                }
                None => false,
            },
        };
        if !changed {
            return Ok(GenerationProviderSettingsMutationResult::Unchanged(current.clone()));
        }
        let next = GenerationProviderSettingsRevision::new(current.revision().get() + 1).unwrap();
        *current = GenerationProviderSettingsSnapshot::try_new(next, bindings).unwrap();
        Ok(GenerationProviderSettingsMutationResult::Committed(current.clone()))
    }
}

fn mock_contracts() -> Arc<Vec<GenerationProviderContract>> {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    Arc::new(vec![GenerationProviderContract::from_provider(&provider)])
}

fn default_snapshot() -> GenerationProviderSettingsSnapshot {
    GenerationProviderSettingsSnapshot::try_new(
        revision(1),
        vec![
            image_binding(),
            binding(
                "video.cinematic_image_animation",
                GenerationTaskRequestKind::Video,
                "mock.video.cinematic-image-animation.v1",
            ),
            binding(
                "speech.multilingual_narration",
                GenerationTaskRequestKind::Voice,
                "mock.voice.multilingual-narration.v1",
            ),
        ],
    )
    .unwrap()
}

fn image_binding() -> GenerationProviderSettingsBinding {
    binding(
        "image.high_quality_general",
        GenerationTaskRequestKind::Image,
        "mock.image.high-quality-general.v1",
    )
}

fn binding(
    profile_id: &str,
    generation_kind: GenerationTaskRequestKind,
    route_id: &str,
) -> GenerationProviderSettingsBinding {
    GenerationProviderSettingsBinding::new(
        GenerationProfileRef::new(
            GenerationProfileId::try_new(profile_id).unwrap(),
            GenerationProfileVersion::try_new(1).unwrap(),
        ),
        generation_kind,
        GenerationProviderId::try_new("mock").unwrap(),
        GenerationProviderRouteId::try_new(route_id).unwrap(),
    )
}

fn revision(value: u64) -> GenerationProviderSettingsRevision {
    GenerationProviderSettingsRevision::new(value).unwrap()
}

fn mutation_matches(
    mutation: &GenerationProviderSettingsMutation,
    binding: &GenerationProviderSettingsBinding,
) -> bool {
    match mutation {
        GenerationProviderSettingsMutation::SetBinding(candidate) => {
            candidate.profile_ref() == binding.profile_ref()
                && candidate.generation_kind() == binding.generation_kind()
        }
        GenerationProviderSettingsMutation::RemoveBinding { profile_ref, generation_kind } => {
            profile_ref == binding.profile_ref() && *generation_kind == binding.generation_kind()
        }
    }
}
