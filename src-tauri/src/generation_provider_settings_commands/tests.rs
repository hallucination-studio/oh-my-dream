use tempfile::tempdir;

use super::*;
use crate::composition::{DesktopApplicationPaths, DesktopCompositionRoot};

#[tokio::test]
async fn commands_get_apply_noop_conflict_and_expose_only_mock_choices() {
    let directory = tempdir().unwrap();
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .unwrap();

    let initial = generation_provider_settings_get_with_dependencies(&dependencies).await.unwrap();
    assert_eq!(initial.settings_revision, "1");
    assert_eq!(initial.profiles.len(), 3);
    assert!(initial.profiles.iter().all(|profile| {
        profile.selected_binding.is_some()
            && profile.provider_choices.len() == 1
            && profile.provider_choices[0].provider_id == "mock"
            && profile.provider_choices[0].routes.len() == 1
    }));
    let encoded = serde_json::to_string(&initial).unwrap();
    for prohibited in
        ["credential", "account", "endpoint", "native_model", "remote_task", "supports_"]
    {
        assert!(!encoded.contains(prohibited), "Settings leaked {prohibited}");
    }

    let image = initial.profiles.iter().find(|profile| profile.generation_kind == "image").unwrap();
    let removal = GenerationProviderSettingsActionDto::RemoveBinding {
        profile_ref: image.profile_ref.clone(),
        generation_kind: image.generation_kind.clone(),
    };
    let removed = generation_provider_settings_apply_with_dependencies(
        GenerationProviderSettingsApplyRequestDto {
            expected_settings_revision: "1".to_owned(),
            action: removal.clone(),
        },
        &dependencies,
    )
    .await
    .unwrap();
    assert_eq!(removed.settings_revision, "2");
    assert!(
        removed
            .profiles
            .iter()
            .find(|profile| profile.generation_kind == "image")
            .unwrap()
            .selected_binding
            .is_none()
    );

    let unchanged = generation_provider_settings_apply_with_dependencies(
        GenerationProviderSettingsApplyRequestDto {
            expected_settings_revision: "2".to_owned(),
            action: removal.clone(),
        },
        &dependencies,
    )
    .await
    .unwrap();
    assert_eq!(unchanged.settings_revision, "2");
    assert_eq!(
        generation_provider_settings_apply_with_dependencies(
            GenerationProviderSettingsApplyRequestDto {
                expected_settings_revision: "1".to_owned(),
                action: removal,
            },
            &dependencies,
        )
        .await
        .unwrap_err()
        .code,
        "generation_provider_settings.revision_conflict"
    );
}

#[tokio::test]
async fn apply_rejects_noncanonical_values_and_unknown_provider_tuple() {
    let directory = tempdir().unwrap();
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .unwrap();
    for request in [
        GenerationProviderSettingsApplyRequestDto {
            expected_settings_revision: "01".to_owned(),
            action: GenerationProviderSettingsActionDto::RemoveBinding {
                profile_ref: "image.high_quality_general@1".to_owned(),
                generation_kind: "image".to_owned(),
            },
        },
        GenerationProviderSettingsApplyRequestDto {
            expected_settings_revision: "1".to_owned(),
            action: GenerationProviderSettingsActionDto::SetBinding {
                profile_ref: "image.high_quality_general@1".to_owned(),
                generation_kind: "image".to_owned(),
                provider_id: "vendor".to_owned(),
                route_id: "mock.image.high-quality-general.v1".to_owned(),
            },
        },
    ] {
        assert_eq!(
            generation_provider_settings_apply_with_dependencies(request, &dependencies)
                .await
                .unwrap_err()
                .code,
            "generation_provider_settings.invalid_request"
        );
    }
}
