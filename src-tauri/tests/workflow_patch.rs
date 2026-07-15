use engine::{
    CapabilityRef, InputBinding, NodeRef, Workflow, WorkflowNode, WorkflowPatchOperation,
    WorkflowReadinessBlocker,
};
use oh_my_dream_tauri::assistant_operations::RequestContext;
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::WorkflowCommitRequest;
use oh_my_dream_tauri::workflow_patch_operation::{WorkflowApplyPatchInput, WorkflowPatchService};
use serde_json::{Map, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

fn state_with_project() -> (tempfile::TempDir, AppState) {
    let root = tempdir().expect("asset root");
    let state = AppState::from_asset_root(root.path()).expect("app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("project", "Project")
        .expect("project");
    (root, state)
}

fn context(request_id: &str) -> RequestContext {
    RequestContext::new("project", "session", request_id, 1, None)
}

fn add(alias: &str, id: &str) -> WorkflowPatchOperation {
    WorkflowPatchOperation::AddNode {
        alias: alias.to_owned(),
        capability: CapabilityRef::new(id, "1.0"),
        params: Map::new(),
        position: None,
    }
}

#[test]
fn workflow_apply_patch_creates_one_revision_and_one_undo_unit() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let output = service
        .apply(
            &context("request-1"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![WorkflowPatchOperation::AddNode {
                    alias: "prompt".to_owned(),
                    capability: CapabilityRef::new("TextPrompt", "1.0"),
                    params: Map::from_iter([(String::from("text"), json!("hello"))]),
                    position: Some([10.0, 20.0]),
                }],
            },
        )
        .expect("patch should commit");

    let head = output.workflow_head.expect("first non-empty patch has a head");
    assert_eq!(head.revision, 1);
    assert!(output.changed);
    assert_eq!(output.aliases[0].alias, "prompt");
    assert_eq!(head.workflow["nodes"][0]["type"], "Text");
    assert_eq!(head.workflow["nodes"][0]["params"]["mode"], "literal");
    assert_eq!(head.workflow["nodes"][0]["position"], json!([10.0, 20.0]));
}

#[test]
fn workflow_apply_patch_resolves_later_aliases_and_preserves_ordered_blockers() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let output = service
        .apply(
            &context("request-ordered"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![
                    add("video", "ImageToVideo"),
                    add("concat", "VideoConcat"),
                    WorkflowPatchOperation::SetInput {
                        node: NodeRef::Alias { alias: "concat".to_owned() },
                        input: "clips".to_owned(),
                        binding: InputBinding::ordered_many(vec![NodeRef::Alias {
                            alias: "video".to_owned(),
                        }]),
                    },
                ],
            },
        )
        .expect("incomplete ordered binding is a readiness blocker");

    assert_eq!(output.workflow_head.as_ref().expect("head").revision, 1);
    assert!(output.readiness_blockers.iter().any(|blocker| {
        blocker.code == "INPUT_CARDINALITY_UNSATISFIED" && blocker.pointer.contains("clips")
    }));
    let inputs = &output.workflow_head.expect("head").workflow["nodes"][1]["inputs"];
    assert_eq!(inputs["clips"]["kind"], "ordered_many");
}

#[test]
fn workflow_apply_patch_failure_does_not_write_a_partial_head() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let error = service
        .apply(
            &context("request-failure"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![
                    add("prompt", "TextPrompt"),
                    WorkflowPatchOperation::ReplaceParams {
                        node: NodeRef::Alias { alias: "prompt".to_owned() },
                        params: Map::from_iter([(String::from("unknown"), json!(true))]),
                    },
                ],
            },
        )
        .expect_err("unknown params must reject the whole patch");

    assert_eq!(error.code, "CAPABILITY_PARAMS_INVALID");
    assert_eq!(error.operation_index, Some(1));
    assert!(state.workflow_authority.load_head("project").expect("load head").is_none());
}

#[test]
fn workflow_evaluate_patch_returns_engine_findings_without_mutating_authority() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);

    let output = service
        .evaluate(
            &context("evaluate"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![add("video", "ImageToVideo")],
            },
        )
        .expect("incomplete workflow should evaluate with blockers");

    assert_eq!(output.base_revision, None);
    assert_eq!(output.workflow.nodes.len(), 1);
    assert!(!output.readiness_blockers.is_empty());
    assert!(state.workflow_authority.load_head("project").expect("load head").is_none());
}

#[test]
fn workflow_evaluate_patch_rejects_a_stale_base_before_proposing_changes() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    service
        .apply(
            &context("commit"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![add("prompt", "TextPrompt")],
            },
        )
        .expect("create head");

    let error = service
        .evaluate(
            &context("stale-evaluate"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![add("other", "TextPrompt")],
            },
        )
        .expect_err("absent revision is stale once a head exists");

    assert_eq!(error.code, "WORKFLOW_REVISION_CONFLICT");
    assert_eq!(error.current_revision, Some(1));
}

#[test]
fn workflow_apply_patch_reports_current_revision_for_stale_writes() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    service
        .apply(
            &context("request-first"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![add("prompt", "TextPrompt")],
            },
        )
        .expect("first patch");
    let error = service
        .apply(
            &context("request-stale"),
            WorkflowApplyPatchInput {
                expected_revision: Some(0),
                operations: vec![add("other", "TextPrompt")],
            },
        )
        .expect_err("stale patch must fail");

    assert_eq!(error.code, "WORKFLOW_REVISION_CONFLICT");
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(
        state.workflow_authority.load_head("project").expect("head").expect("head").revision,
        1
    );
}

#[test]
fn workflow_apply_patch_removes_incident_bindings_atomically() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let output = service
        .apply(
            &context("request-remove"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![
                    add("first", "ImageToVideo"),
                    add("second", "ImageToVideo"),
                    add("concat", "VideoConcat"),
                    WorkflowPatchOperation::SetInput {
                        node: NodeRef::Alias { alias: "concat".to_owned() },
                        input: "clips".to_owned(),
                        binding: InputBinding::ordered_many(vec![
                            NodeRef::Alias { alias: "first".to_owned() },
                            NodeRef::Alias { alias: "second".to_owned() },
                        ]),
                    },
                    WorkflowPatchOperation::RemoveNode {
                        node: NodeRef::Alias { alias: "first".to_owned() },
                    },
                ],
            },
        )
        .expect("remove should commit with a new blocker");

    let head = output.workflow_head.expect("head");
    assert_eq!(head.workflow["nodes"].as_array().expect("nodes").len(), 2);
    assert_eq!(
        head.workflow["nodes"][1]["inputs"]["clips"]["sources"].as_array().expect("sources").len(),
        1
    );
    assert!(output.readiness_blockers.iter().any(|blocker: &WorkflowReadinessBlocker| {
        blocker.code == "INPUT_CARDINALITY_UNSATISFIED"
    }));
}

#[test]
fn workflow_apply_patch_operation_registration_is_non_strict_only_for_params() {
    let (_root, state) = state_with_project();
    let registration = Arc::new(WorkflowPatchService::from_state(&state))
        .operation_registration()
        .expect("register workflow patch operation");
    assert_eq!(registration.id(), "workflow_apply_patch");
    assert!(!registration.sdk_strict_json_schema());
    assert_eq!(registration.input_schema()["additionalProperties"], json!(false));
    assert_eq!(
        registration.input_schema()["properties"]["operations"]["items"]["oneOf"][0]["properties"]
            ["params"]["additionalProperties"],
        json!(true)
    );
}

#[test]
fn workflow_apply_patch_attributes_type_mismatch_to_the_set_input_operation() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let error = service
        .apply(
            &context("request-type-mismatch"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![
                    add("prompt", "TextPrompt"),
                    add("video", "ImageToVideo"),
                    WorkflowPatchOperation::SetInput {
                        node: NodeRef::Alias { alias: "video".to_owned() },
                        input: "image".to_owned(),
                        binding: InputBinding::single(NodeRef::Alias {
                            alias: "prompt".to_owned(),
                        }),
                    },
                    WorkflowPatchOperation::SetPosition {
                        node: NodeRef::Alias { alias: "video".to_owned() },
                        position: [1.0, 2.0],
                    },
                ],
            },
        )
        .expect_err("incompatible connection must reject the patch");

    assert_eq!(error.code, "INPUT_OUTPUT_TYPE_MISMATCH");
    assert_eq!(error.operation_index, Some(2));
    assert_eq!(error.pointer, "/operations/2/binding/source");
    assert!(state.workflow_authority.load_head("project").expect("head").is_none());
}

#[test]
fn workflow_apply_patch_attributes_cycles_to_the_set_input_operation() {
    let (_root, state) = state_with_project();
    let service = WorkflowPatchService::from_state(&state);
    let error = service
        .apply(
            &context("request-cycle"),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![
                    add("concat", "VideoConcat"),
                    WorkflowPatchOperation::SetInput {
                        node: NodeRef::Alias { alias: "concat".to_owned() },
                        input: "clips".to_owned(),
                        binding: InputBinding::ordered_many(vec![
                            NodeRef::Alias { alias: "concat".to_owned() },
                            NodeRef::Alias { alias: "concat".to_owned() },
                        ]),
                    },
                    WorkflowPatchOperation::SetPosition {
                        node: NodeRef::Alias { alias: "concat".to_owned() },
                        position: [1.0, 2.0],
                    },
                ],
            },
        )
        .expect_err("cyclic connection must reject the patch");

    assert_eq!(error.code, "WORKFLOW_CYCLE");
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(error.pointer, "/operations/1/binding");
    assert!(state.workflow_authority.load_head("project").expect("head").is_none());
}

#[test]
fn workflow_apply_patch_reports_existing_contract_errors_as_workflow_pointers() {
    let (_root, state) = state_with_project();
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "project",
            None,
            "legacy-head",
            "legacy-hash",
            Workflow {
                version: "1.0".to_owned(),
                project_id: "project".to_owned(),
                nodes: vec![WorkflowNode {
                    id: "legacy".to_owned(),
                    type_id: "MissingCapability".to_owned(),
                    contract_version: "1.0".to_owned(),
                    params: Map::new(),
                    inputs: BTreeMap::new(),
                    position: None,
                }],
            },
        ))
        .expect("persist legacy head");
    let service = WorkflowPatchService::from_state(&state);
    let error = service
        .apply(
            &context("request-existing-invalid"),
            WorkflowApplyPatchInput {
                expected_revision: Some(1),
                operations: vec![WorkflowPatchOperation::SetPosition {
                    node: NodeRef::Id { id: "legacy".to_owned() },
                    position: [1.0, 2.0],
                }],
            },
        )
        .expect_err("missing exact registration must remain a Workflow error");

    assert_eq!(error.code, "CAPABILITY_VERSION_UNAVAILABLE");
    assert_eq!(error.pointer, "/nodes/0/params");
    assert_eq!(error.operation_index, None);
    assert_eq!(error.current_revision, Some(1));
}
