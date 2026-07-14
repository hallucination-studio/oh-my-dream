use engine::{
    CapabilityContract, CapabilityPresentation, CapabilityRef, CapabilityRegistration,
    CapabilityRegistry, CapabilityRegistryError, CapabilitySelector, NodeParams,
};

#[test]
fn selectors_resolve_different_capabilities_under_one_modality() {
    let mut registry = CapabilityRegistry::default();
    let image = CapabilitySelector::new("Video", "image");
    let concat = CapabilitySelector::new("Video", "concat");

    registry
        .register_selector_current(registration("ImageToVideo", "1.0", image.clone()))
        .expect("image mode should register");
    registry
        .register_selector_current(registration("VideoConcat", "1.0", concat.clone()))
        .expect("concat mode should register");

    assert_eq!(
        registry.current_for_selector(&image),
        Some(CapabilityRef::new("ImageToVideo", "1.0"))
    );
    assert_eq!(
        registry.current_for_selector(&concat),
        Some(CapabilityRef::new("VideoConcat", "1.0"))
    );
}

#[test]
fn selector_can_advance_only_within_the_same_capability_id() {
    let mut registry = CapabilityRegistry::default();
    let selector = CapabilitySelector::new("Video", "image");

    registry
        .register_selector_current(registration("ImageToVideo", "1.0", selector.clone()))
        .expect("first version should register");
    registry
        .register_selector_current(registration("ImageToVideo", "2.0", selector.clone()))
        .expect("later version should register");

    assert_eq!(
        registry.current_for_selector(&selector),
        Some(CapabilityRef::new("ImageToVideo", "2.0"))
    );
}

#[test]
fn rejected_selector_rebind_does_not_register_the_exact_capability() {
    let mut registry = CapabilityRegistry::default();
    let selector = CapabilitySelector::new("Video", "image");
    registry
        .register_selector_current(registration("ImageToVideo", "1.0", selector.clone()))
        .expect("first capability should register");

    let result = registry.register_selector_current(registration(
        "UnexpectedVideoGenerator",
        "1.0",
        selector.clone(),
    ));

    assert_eq!(
        result,
        Err(CapabilityRegistryError::SelectorRebind {
            selector: selector.clone(),
            registered_id: "ImageToVideo".to_owned(),
            attempted_id: "UnexpectedVideoGenerator".to_owned(),
        })
    );
    assert_eq!(
        registry.current_for_selector(&selector),
        Some(CapabilityRef::new("ImageToVideo", "1.0"))
    );
    assert!(
        registry
            .resolve(&CapabilityRef::new("UnexpectedVideoGenerator", "1.0"))
            .is_err()
    );
}

#[test]
fn non_current_registration_also_reserves_the_selector_capability_id() {
    let mut registry = CapabilityRegistry::default();
    let selector = CapabilitySelector::new("Video", "image");
    registry
        .register(registration("ImageToVideo", "1.0", selector.clone()))
        .expect("non-current exact version should register");

    let result = registry.register_selector_current(registration(
        "UnexpectedVideoGenerator",
        "1.0",
        selector.clone(),
    ));

    assert_eq!(
        result,
        Err(CapabilityRegistryError::SelectorRebind {
            selector,
            registered_id: "ImageToVideo".to_owned(),
            attempted_id: "UnexpectedVideoGenerator".to_owned(),
        })
    );
    assert!(
        registry
            .resolve(&CapabilityRef::new("UnexpectedVideoGenerator", "1.0"))
            .is_err()
    );
}

#[test]
fn exact_registration_projects_its_declared_selector() {
    let mut registry = CapabilityRegistry::default();
    let selector = CapabilitySelector::new("Image", "text");
    let reference = CapabilityRef::new("TextToImage", "1.0");
    registry
        .register_selector_current(registration(
            &reference.id,
            &reference.version,
            selector.clone(),
        ))
        .expect("capability should register");

    let registration = registry
        .resolve(&reference)
        .expect("exact ref should resolve");

    assert_eq!(registration.selector(), Some(&selector));
}

fn registration(id: &str, version: &str, selector: CapabilitySelector) -> CapabilityRegistration {
    CapabilityRegistration::new(
        CapabilityContract::new(
            CapabilityRef::new(id, version),
            Vec::new(),
            Vec::new(),
            serde_json::json!({ "type": "object" }),
            NodeParams::new(),
            Vec::new(),
        ),
        CapabilityPresentation::new(id, id, "test", Vec::new()),
        Box::new(|params| Ok(params.clone())),
        Box::new(|_| unreachable!("factory is not used by registry tests")),
    )
    .with_selector(selector)
}
