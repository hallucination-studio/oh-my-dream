use engine::{CapabilityRef, WorkflowNode};
use nodes::{
    CapabilityNodeStatus, DegradedCapabilityReason, migrate_legacy_node, resolve_workflow_node,
};
use oh_my_dream_tauri::commands::{
    get_capability_bundles_with_state, get_capability_catalog_with_state,
    search_capabilities_with_state,
};
use oh_my_dream_tauri::dto::{
    CapabilityAvailabilityDto, CapabilityCardinalityDto, CapabilityCatalogDto,
};
use oh_my_dream_tauri::state::AppState;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn capability_catalog_keeps_contract_presentation_and_status_independent() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let catalog = get_capability_catalog_with_state(&state).expect("capability catalog");

    let refs = catalog
        .capabilities
        .iter()
        .map(|entry| {
            (entry.contract.reference.id.as_str(), entry.contract.reference.version.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        refs,
        vec![
            ("ImageToVideo", "1.0"),
            ("TextPrompt", "1.0"),
            ("TextToAudio", "1.0"),
            ("TextToImage", "1.0"),
            ("VideoConcat", "1.0"),
        ]
    );

    let concat = catalog
        .capabilities
        .iter()
        .find(|entry| entry.contract.reference.id == "VideoConcat")
        .expect("concat catalog entry");
    assert_eq!(concat.contract.inputs[0].port_type, "video");
    assert_eq!(
        concat.contract.inputs[0].cardinality,
        CapabilityCardinalityDto::Many { minimum: 2, maximum: None }
    );
    assert_eq!(concat.presentation.label, "Video Concat");
    assert_eq!(concat.status.availability, CapabilityAvailabilityDto::Available);
    assert_eq!(concat.status.status_revision, 0);
    assert!(concat.status.reason.is_none());

    let image = catalog
        .capabilities
        .iter()
        .find(|entry| entry.contract.reference.id == "TextToImage")
        .expect("image catalog entry");
    assert_eq!(image.status.provider_health.as_deref(), Some("unknown"));

    let encoded = serde_json::to_value(&catalog).expect("serialize catalog");
    let decoded =
        serde_json::from_value::<CapabilityCatalogDto>(encoded).expect("deserialize catalog");
    assert_eq!(decoded, catalog);
    assert_eq!(
        catalog.capabilities[0].contract.reference,
        oh_my_dream_tauri::dto::CapabilityRefDto {
            id: "ImageToVideo".to_owned(),
            version: "1.0".to_owned(),
        }
    );
}

#[test]
fn frozen_legacy_fixture_preserves_aliases_and_defaults() {
    let fixtures = serde_json::from_str::<Vec<LegacyFixture>>(include_str!(
        "fixtures/legacy_capabilities.json"
    ))
    .expect("legacy fixture");

    for fixture in fixtures {
        let migrated = migrate_legacy_node(fixture.raw).expect("migrate legacy node");
        assert_eq!(migrated.contract_version, "1.0", "fixture {}", fixture.name);
        assert_eq!(migrated.params, fixture.expected_params, "fixture {}", fixture.name);
    }
}

#[test]
fn missing_exact_version_reopens_as_preserved_degraded_node() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let node = WorkflowNode {
        id: "image".to_owned(),
        type_id: "TextToImage".to_owned(),
        contract_version: "9.9".to_owned(),
        params: serde_json::Map::from_iter([(String::from("model"), json!("old-image"))]),
        inputs: BTreeMap::new(),
        position: Some([12.0, 24.0]),
    };

    let resolution = resolve_workflow_node(&state.registry, &node);
    assert_eq!(resolution.node.id, node.id);
    assert_eq!(resolution.node.contract_version, "9.9");
    assert_eq!(resolution.node.params, node.params);
    assert_eq!(resolution.node.position, node.position);
    assert!(matches!(
        resolution.status,
        CapabilityNodeStatus::Degraded(DegradedCapabilityReason::MissingExactVersion {
            reference: CapabilityRef { id, version }
        }) if id == "TextToImage" && version == "9.9"
    ));
}

#[test]
fn palette_search_is_paged_and_only_returns_current_summaries() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let first = search_capabilities_with_state("video".to_owned(), None, None, Some(1), &state)
        .expect("search capability summaries");

    assert_eq!(first.capabilities.len(), 1);
    assert_eq!(first.capabilities[0].reference.id, "ImageToVideo");
    assert!(first.capabilities[0].presentation.search_terms.iter().any(|term| term == "video"));
    assert!(first.capabilities[0].status.reason.is_none());
    assert!(first.capabilities[0].status.status_revision == 0);
    assert_eq!(first.next_cursor.as_deref(), Some("1"));

    let second = search_capabilities_with_state(
        "video".to_owned(),
        None,
        first.next_cursor,
        Some(10),
        &state,
    )
    .expect("search next capability page");
    assert_eq!(second.capabilities.len(), 1);
    assert_eq!(second.capabilities[0].reference.id, "VideoConcat");
    assert!(second.next_cursor.is_none());
}

#[test]
fn exact_bundle_batch_preserves_unknown_refs_as_degraded_placeholders() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let result = get_capability_bundles_with_state(
        vec![
            oh_my_dream_tauri::dto::CapabilityRefDto {
                id: "TextPrompt".to_owned(),
                version: "1.0".to_owned(),
            },
            oh_my_dream_tauri::dto::CapabilityRefDto {
                id: "TextPrompt".to_owned(),
                version: "9.9".to_owned(),
            },
        ],
        &state,
    )
    .expect("load exact bundles");

    assert_eq!(result.capabilities.len(), 2);
    assert!(result.capabilities[0].contract.is_some());
    assert!(result.capabilities[1].contract.is_none());
    assert_eq!(result.capabilities[1].status.availability, CapabilityAvailabilityDto::Degraded);
    assert!(result.capabilities[1].status.reason.is_some());
}

#[derive(Debug, Deserialize)]
struct LegacyFixture {
    name: String,
    raw: Value,
    expected_params: serde_json::Map<String, Value>,
}
