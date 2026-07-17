use engine::node_capability::NodeCapabilityGenerationProfileRefParameterValue;
use nodes::{
    GenerationProfileCatalog, GenerationProfileError, GenerationProfileId,
    GenerationProfileLifecycleState, GenerationProfileRef, GenerationProfileVersion,
};

#[test]
fn profile_ref_round_trips_through_the_engine_boundary_value() {
    let profile_ref = GenerationProfileRef::new(
        GenerationProfileId::try_new("image.high_quality_general").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    );

    let boundary = profile_ref.to_node_capability_parameter_value().unwrap();
    let restored =
        GenerationProfileRef::try_from_node_capability_parameter_value(&boundary).unwrap();

    assert_eq!(restored, profile_ref);
    assert_eq!(profile_ref.to_string(), "image.high_quality_general@1");
}

#[test]
fn invalid_profile_identity_is_rejected_without_catalog_lookup() {
    assert_eq!(
        GenerationProfileId::try_new("fal").unwrap_err(),
        GenerationProfileError::InvalidProfileRef
    );
    assert!(NodeCapabilityGenerationProfileRefParameterValue::new("fal", 1).is_err());
}

#[test]
fn frozen_catalog_contains_only_the_three_active_mvp_profiles() {
    let catalog = GenerationProfileCatalog::frozen_mvp().unwrap();
    for (profile_id, display_name, capability_ref) in [
        ("image.high_quality_general", "High Quality Image", "image.generate_from_text@1.0"),
        (
            "speech.multilingual_narration",
            "Multilingual Narration",
            "audio.synthesize_speech_from_text@1.0",
        ),
        (
            "video.cinematic_image_animation",
            "Cinematic Image Animation",
            "video.generate_from_image@1.0",
        ),
    ] {
        let profile_ref = GenerationProfileRef::new(
            GenerationProfileId::try_new(profile_id).unwrap(),
            GenerationProfileVersion::try_new(1).unwrap(),
        );
        let definition = catalog.find_generation_profile(&profile_ref).unwrap();
        assert_eq!(definition.display_name().as_str(), display_name);
        assert_eq!(definition.lifecycle_state(), GenerationProfileLifecycleState::Active);
        assert_eq!(
            definition
                .compatible_capabilities()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![capability_ref]
        );
    }
    let unknown = GenerationProfileRef::new(
        GenerationProfileId::try_new("image.not_in_catalog").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    );
    assert_eq!(
        catalog.find_generation_profile(&unknown).unwrap_err(),
        GenerationProfileError::ProfileNotFound
    );
}
