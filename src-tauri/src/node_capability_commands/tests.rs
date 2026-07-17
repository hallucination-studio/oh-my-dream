use tempfile::tempdir;

use super::*;
use crate::composition::{DesktopApplicationPaths, DesktopCompositionRoot};

#[tokio::test]
async fn lists_exact_seven_contracts_and_only_compatible_profile() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("dependencies");

    let contracts = node_capability_list_with_dependencies(&dependencies);
    let refs = contracts
        .iter()
        .map(|contract| {
            format!("{}@{}", contract.capability_ref.id, contract.capability_ref.version)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        refs,
        vec![
            "audio.read_asset@1.0",
            "audio.synthesize_speech_from_text@1.0",
            "image.generate_from_text@1.0",
            "image.read_asset@1.0",
            "text.provide_literal@1.0",
            "video.generate_from_image@1.0",
            "video.read_asset@1.0",
        ]
    );

    let profiles = generation_profile_list_with_dependencies(
        GenerationProfileListForCapabilityRequestDto {
            capability_id: "image.generate_from_text".to_owned(),
            capability_version: "1.0".to_owned(),
        },
        &dependencies,
    )
    .await
    .expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_ref, "image.high_quality_general@1");
    assert_eq!(profiles[0].availability.state, "available");
    assert_eq!(profiles[0].availability.reason, None);
}

#[tokio::test]
async fn rejects_unknown_or_noncanonical_capability_ref() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("dependencies");

    for request in [
        GenerationProfileListForCapabilityRequestDto {
            capability_id: "image.unknown".to_owned(),
            capability_version: "1.0".to_owned(),
        },
        GenerationProfileListForCapabilityRequestDto {
            capability_id: "image.generate_from_text".to_owned(),
            capability_version: "01.0".to_owned(),
        },
    ] {
        assert_eq!(
            generation_profile_list_with_dependencies(request, &dependencies)
                .await
                .expect_err("invalid")
                .code,
            "node_capability.invalid_request"
        );
    }
}
