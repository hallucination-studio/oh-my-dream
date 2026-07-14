use engine::{CapabilityRef, Workflow, WorkflowNode};
use oh_my_dream_tauri::assistant_operations::{OperationDispatchError, RequestContext};
use oh_my_dream_tauri::capability_discovery::{
    CapabilityDescribeInput, CapabilityDiscovery, CapabilityDiscoveryError, CapabilitySearchInput,
    MAX_DESCRIBE_REFS_PER_CALL, MAX_SEARCH_RESULTS,
};
use oh_my_dream_tauri::dto::{CapabilityAvailabilityDto, CapabilityRefDto};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::WorkflowCommitRequest;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

fn context(request_id: &str) -> RequestContext {
    RequestContext::new("default", "session-1", request_id, 1, None)
}

#[test]
fn search_requires_a_goal_and_returns_only_bounded_current_refs() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let discovery = CapabilityDiscovery::from_state(&state);

    assert_eq!(
        discovery
            .search(
                &context("empty-goal"),
                CapabilitySearchInput { query: "  ".to_owned(), kinds: None },
            )
            .expect_err("empty search goal must fail"),
        CapabilityDiscoveryError::EmptyGoal
    );

    let output = discovery
        .search(
            &context("search-video"),
            CapabilitySearchInput { query: "video".to_owned(), kinds: None },
        )
        .expect("video search");
    assert!(output.capabilities.len() <= MAX_SEARCH_RESULTS);
    assert!(!output.capabilities.is_empty());
    assert!(output.capabilities.iter().all(|result| {
        result.reference.version == "1.0"
            && result.status.availability == CapabilityAvailabilityDto::Available
    }));
    assert!(output.capabilities.iter().any(|result| result.reference.id == "ImageToVideo"));
    assert!(output.capabilities.iter().all(|result| !result.selector.type_id.is_empty()));
}

#[test]
fn describe_accepts_search_refs_but_rejects_new_undiscovered_refs() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let discovery = CapabilityDiscovery::from_state(&state);
    let search_context = context("describe-admission");
    let search = discovery
        .search(&search_context, CapabilitySearchInput { query: "image".to_owned(), kinds: None })
        .expect("image search");
    let selected = search.capabilities[0].reference.clone();

    let described = discovery
        .describe(&search_context, CapabilityDescribeInput { refs: vec![selected.clone()] })
        .expect("describe admitted ref");
    assert_eq!(described.capabilities[0].reference, selected);
    assert!(described.capabilities[0].contract.is_some());

    let error = discovery
        .describe(
            &context("undiscovered"),
            CapabilityDescribeInput {
                refs: vec![oh_my_dream_tauri::dto::CapabilityRefDto {
                    id: "TextPrompt".to_owned(),
                    version: "1.0".to_owned(),
                }],
            },
        )
        .expect_err("new refs must come from search");
    assert!(
        matches!(error, CapabilityDiscoveryError::NotAdmitted { reference } if reference == CapabilityRef::new("TextPrompt", "1.0"))
    );
}

#[test]
fn persisted_missing_version_is_describable_as_degraded() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("default", "Default")
        .expect("create project");
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "default",
            None,
            "persisted-degraded",
            "hash-persisted-degraded",
            Workflow {
                version: "1.0".to_owned(),
                project_id: "default".to_owned(),
                nodes: vec![WorkflowNode {
                    id: "image".to_owned(),
                    type_id: "TextToImage".to_owned(),
                    contract_version: "9.9".to_owned(),
                    params: serde_json::Map::from_iter([(String::from("model"), json!("old"))]),
                    inputs: BTreeMap::new(),
                    position: None,
                }],
            },
        ))
        .expect("persist degraded Workflow head");
    let discovery = CapabilityDiscovery::from_state(&state);
    let output = discovery
        .describe(
            &context("persisted-degraded"),
            CapabilityDescribeInput {
                refs: vec![oh_my_dream_tauri::dto::CapabilityRefDto {
                    id: "TextToImage".to_owned(),
                    version: "9.9".to_owned(),
                }],
            },
        )
        .expect("persisted missing ref should remain describable");
    let description = &output.capabilities[0];
    assert!(description.contract.is_none());
    assert_eq!(description.status.availability, CapabilityAvailabilityDto::Degraded);
    assert!(description.status.reason.as_deref().is_some_and(|reason| reason.contains("migrate")));
}

#[test]
fn persisted_canonical_missing_version_resolves_to_exact_degraded_ref() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("default", "Default")
        .expect("create project");
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "default",
            None,
            "persisted-canonical-degraded",
            "hash-persisted-canonical-degraded",
            Workflow {
                version: "1.0".to_owned(),
                project_id: "default".to_owned(),
                nodes: vec![WorkflowNode {
                    id: "video".to_owned(),
                    type_id: "Video".to_owned(),
                    contract_version: "9.9".to_owned(),
                    params: serde_json::Map::from_iter([(String::from("mode"), json!("image"))]),
                    inputs: BTreeMap::new(),
                    position: None,
                }],
            },
        ))
        .expect("persist canonical degraded Workflow head");
    let discovery = CapabilityDiscovery::from_state(&state);
    let output = discovery
        .describe(
            &context("persisted-canonical-degraded"),
            CapabilityDescribeInput {
                refs: vec![CapabilityRefDto {
                    id: "ImageToVideo".to_owned(),
                    version: "9.9".to_owned(),
                }],
            },
        )
        .expect("canonical missing ref should remain describable");

    assert_eq!(output.capabilities[0].reference.id, "ImageToVideo");
    assert!(output.capabilities[0].selector.is_none());
    assert_eq!(output.capabilities[0].status.availability, CapabilityAvailabilityDto::Degraded);
}

#[test]
fn describe_enforces_eight_distinct_refs_per_invocation() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("default", "Default")
        .expect("create project");
    let refs = (0..9)
        .map(|index| CapabilityRefDto {
            id: format!("LegacyCapability{index}"),
            version: "1.0".to_owned(),
        })
        .collect::<Vec<_>>();
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: refs
            .iter()
            .enumerate()
            .map(|(index, reference)| WorkflowNode {
                id: format!("legacy-{index}"),
                type_id: reference.id.clone(),
                contract_version: reference.version.clone(),
                params: serde_json::Map::new(),
                inputs: BTreeMap::new(),
                position: None,
            })
            .collect(),
    };
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "default",
            None,
            "describe-budget-workflow",
            "hash-describe-budget-workflow",
            workflow,
        ))
        .expect("persist Workflow head");
    let discovery = CapabilityDiscovery::from_state(&state);
    let request = context("describe-budget");

    for chunk in refs[..8].chunks(3) {
        discovery
            .describe(&request, CapabilityDescribeInput { refs: chunk.to_vec() })
            .expect("the first eight persisted refs fit the budget");
    }
    let error = discovery
        .describe(&request, CapabilityDescribeInput { refs: vec![refs[8].clone()] })
        .expect_err("the ninth distinct ref must exceed the invocation budget");
    assert_eq!(error, CapabilityDiscoveryError::DescribeBudgetExceeded { maximum: 8 });
}

#[test]
fn describe_rejects_more_than_three_refs_and_registration_contracts_are_fixed() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let discovery = Arc::new(CapabilityDiscovery::from_state(&state));
    let error = discovery
        .describe(
            &context("too-many"),
            CapabilityDescribeInput {
                refs: (0..=MAX_DESCRIBE_REFS_PER_CALL)
                    .map(|index| oh_my_dream_tauri::dto::CapabilityRefDto {
                        id: format!("capability-{index}"),
                        version: "1.0".to_owned(),
                    })
                    .collect(),
            },
        )
        .expect_err("describe must enforce the per-call ref bound");
    assert_eq!(
        error,
        CapabilityDiscoveryError::TooManyRefs { maximum: MAX_DESCRIBE_REFS_PER_CALL }
    );

    let registrations = discovery
        .operation_registrations()
        .expect("discovery operations should have closed schemas");
    assert_eq!(
        registrations.iter().map(|registration| registration.id()).collect::<Vec<_>>(),
        vec!["capability_search", "capability_describe"]
    );
}

#[tokio::test]
async fn operation_dispatch_returns_structured_discovery_errors() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let registrations = Arc::new(CapabilityDiscovery::from_state(&state))
        .operation_registrations()
        .expect("discovery operations");
    let error = registrations[0]
        .dispatch(&context("operation-empty"), json!({ "query": " ", "kinds": null }))
        .await
        .expect_err("empty goal must be rejected by the operation");
    assert!(matches!(
        error,
        OperationDispatchError::Handler { source, .. } if source.code() == "CAPABILITY_GOAL_REQUIRED"
    ));
}
