use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use tasks::generation_task::{GenerationProviderContract, GenerationTaskRequestKind};

use super::{
    GenerationProviderSettingsError, GenerationProviderSettingsMutation,
    GenerationProviderSettingsMutationResult, GenerationProviderSettingsProfile,
    GenerationProviderSettingsProviderChoice, GenerationProviderSettingsRepositoryInterface,
    GenerationProviderSettingsRevision, GenerationProviderSettingsRouteChoice,
    GenerationProviderSettingsSnapshot, GenerationProviderSettingsView, focused_routes, kind_order,
};

/// Returns the complete sanitized Generation Provider Settings projection.
pub struct GenerationProviderSettingsGetUseCase<R> {
    repository: Arc<R>,
    contracts: Arc<Vec<GenerationProviderContract>>,
}

impl<R> GenerationProviderSettingsGetUseCase<R>
where
    R: GenerationProviderSettingsRepositoryInterface,
{
    /// Uses one repository and immutable safe provider contract collection.
    #[must_use]
    pub const fn new(repository: Arc<R>, contracts: Arc<Vec<GenerationProviderContract>>) -> Self {
        Self { repository, contracts }
    }

    /// Loads and projects current Settings.
    pub async fn get_generation_provider_settings(
        &self,
    ) -> Result<GenerationProviderSettingsView, GenerationProviderSettingsError> {
        let snapshot = self.repository.load_generation_provider_settings_snapshot().await?;
        project(&snapshot, &self.contracts)
    }
}

/// Applies one validated Settings mutation and returns the complete resulting projection.
pub struct GenerationProviderSettingsApplyUseCase<R> {
    repository: Arc<R>,
    contracts: Arc<Vec<GenerationProviderContract>>,
}

impl<R> GenerationProviderSettingsApplyUseCase<R>
where
    R: GenerationProviderSettingsRepositoryInterface,
{
    /// Uses one repository and immutable safe provider contract collection.
    #[must_use]
    pub const fn new(repository: Arc<R>, contracts: Arc<Vec<GenerationProviderContract>>) -> Self {
        Self { repository, contracts }
    }

    /// Validates against safe contracts before asking the repository to compare-and-swap.
    pub async fn apply_generation_provider_settings(
        &self,
        expected_revision: GenerationProviderSettingsRevision,
        mutation: GenerationProviderSettingsMutation,
    ) -> Result<GenerationProviderSettingsView, GenerationProviderSettingsError> {
        validate_mutation(&mutation, &self.contracts)?;
        let snapshot = match self
            .repository
            .apply_generation_provider_settings_mutation(expected_revision, mutation)
            .await?
        {
            GenerationProviderSettingsMutationResult::Committed(snapshot)
            | GenerationProviderSettingsMutationResult::Unchanged(snapshot) => snapshot,
            GenerationProviderSettingsMutationResult::RevisionConflict => {
                return Err(GenerationProviderSettingsError::RevisionConflict);
            }
        };
        project(&snapshot, &self.contracts)
    }
}

fn project(
    snapshot: &GenerationProviderSettingsSnapshot,
    contracts: &[GenerationProviderContract],
) -> Result<GenerationProviderSettingsView, GenerationProviderSettingsError> {
    let provider_ids =
        contracts.iter().map(|contract| contract.provider_id()).collect::<BTreeSet<_>>();
    if provider_ids.len() != contracts.len() {
        return Err(GenerationProviderSettingsError::ContractProjection);
    }
    let mut profiles = BTreeMap::new();
    for contract in contracts {
        for kind in request_kinds() {
            let Some(routes) = focused_routes(contract, kind) else { continue };
            for route in routes {
                for profile_ref in route.compatible_generation_profiles() {
                    let profile = profiles
                        .entry((profile_ref.clone(), kind_order(kind)))
                        .or_insert_with(|| GenerationProviderSettingsProfile {
                            profile_ref: profile_ref.clone(),
                            generation_kind: kind,
                            selected_binding: None,
                            provider_choices: Vec::new(),
                        });
                    add_choice(profile, contract, route)?;
                }
            }
        }
    }
    for binding in snapshot.bindings() {
        let key = (binding.profile_ref().clone(), kind_order(binding.generation_kind()));
        let profile =
            profiles.get_mut(&key).ok_or(GenerationProviderSettingsError::InvalidSnapshot)?;
        if !binding_is_choice(binding, profile) {
            return Err(GenerationProviderSettingsError::InvalidSnapshot);
        }
        profile.selected_binding = Some(binding.clone());
    }
    let mut profiles = profiles.into_values().collect::<Vec<_>>();
    for profile in &mut profiles {
        profile.provider_choices.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        for provider in &mut profile.provider_choices {
            provider.routes.sort_by(|a, b| a.route_id.cmp(&b.route_id));
        }
    }
    Ok(GenerationProviderSettingsView { settings_revision: snapshot.revision(), profiles })
}

fn add_choice(
    profile: &mut GenerationProviderSettingsProfile,
    contract: &GenerationProviderContract,
    route: &tasks::generation_task::GenerationProviderRouteContract,
) -> Result<(), GenerationProviderSettingsError> {
    let provider = match profile
        .provider_choices
        .iter_mut()
        .find(|choice| choice.provider_id == *contract.provider_id())
    {
        Some(provider) => provider,
        None => {
            profile.provider_choices.push(GenerationProviderSettingsProviderChoice {
                provider_id: contract.provider_id().clone(),
                display_name: contract.display_name().as_str().to_owned(),
                routes: Vec::new(),
            });
            profile
                .provider_choices
                .last_mut()
                .ok_or(GenerationProviderSettingsError::ContractProjection)?
        }
    };
    if provider.routes.iter().any(|choice| choice.route_id == *route.route_id()) {
        return Err(GenerationProviderSettingsError::ContractProjection);
    }
    provider.routes.push(GenerationProviderSettingsRouteChoice {
        route_id: route.route_id().clone(),
        display_name: route.display_name().as_str().to_owned(),
    });
    Ok(())
}

fn validate_mutation(
    mutation: &GenerationProviderSettingsMutation,
    contracts: &[GenerationProviderContract],
) -> Result<(), GenerationProviderSettingsError> {
    match mutation {
        GenerationProviderSettingsMutation::SetBinding(binding) => contracts
            .iter()
            .find(|contract| contract.provider_id() == binding.provider_id())
            .and_then(|contract| focused_routes(contract, binding.generation_kind()))
            .and_then(|routes| routes.iter().find(|route| route.route_id() == binding.route_id()))
            .filter(|route| route.compatible_generation_profiles().contains(binding.profile_ref()))
            .map(|_| ())
            .ok_or(GenerationProviderSettingsError::InvalidMutation),
        GenerationProviderSettingsMutation::RemoveBinding { profile_ref, generation_kind } => {
            let exists = contracts.iter().any(|contract| {
                focused_routes(contract, *generation_kind).is_some_and(|routes| {
                    routes
                        .iter()
                        .any(|route| route.compatible_generation_profiles().contains(profile_ref))
                })
            });
            if exists { Ok(()) } else { Err(GenerationProviderSettingsError::InvalidMutation) }
        }
    }
}

fn binding_is_choice(
    binding: &super::GenerationProviderSettingsBinding,
    profile: &GenerationProviderSettingsProfile,
) -> bool {
    profile.provider_choices.iter().any(|provider| {
        provider.provider_id == *binding.provider_id()
            && provider.routes.iter().any(|route| route.route_id == *binding.route_id())
    })
}

const fn request_kinds() -> [GenerationTaskRequestKind; 4] {
    [
        GenerationTaskRequestKind::Text,
        GenerationTaskRequestKind::Image,
        GenerationTaskRequestKind::Video,
        GenerationTaskRequestKind::Voice,
    ]
}
