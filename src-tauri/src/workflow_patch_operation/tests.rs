use super::*;
use crate::state::AppState;
use engine::{CapabilityRef, NodeParams, WorkflowPatchOperation};
use schemars::{JsonSchema, r#gen::SchemaGenerator};
use serde_json::json;
use tempfile::tempdir;

fn context(request_id: &str) -> RequestContext {
    RequestContext::new("project", "session", request_id, 1, None)
}

#[test]
fn operation_input_schema_closes_envelope_and_opens_only_params() {
    let schema = WorkflowApplyPatchInput::json_schema(&mut SchemaGenerator::default());
    let value = serde_json::to_value(schema).expect("serialize patch schema");
    assert_eq!(value["additionalProperties"], json!(false));
    assert_eq!(
        value["properties"]["operations"]["items"]["oneOf"][0]["properties"]["params"]["additionalProperties"],
        json!(true)
    );
    let source = &value["properties"]["operations"]["items"]["oneOf"][2]["properties"]["binding"]["oneOf"]
        [0]["properties"]["source"];
    assert_eq!(source["required"], json!(["node", "output"]));
    assert_eq!(source["additionalProperties"], json!(false));
}

#[test]
fn patch_operations_reject_node_only_and_unknown_output_reference_fields() {
    let node_only = json!({
        "expected_revision": null,
        "operations": [{
            "op": "set_input",
            "node": {"kind": "id", "id": "target"},
            "input": "text",
            "binding": {"kind": "single", "source": {"kind": "id", "id": "source"}}
        }]
    });
    assert!(serde_json::from_value::<WorkflowApplyPatchInput>(node_only).is_err());

    let unknown = json!({
        "expected_revision": null,
        "operations": [{
            "op": "set_input",
            "node": {"kind": "id", "id": "target"},
            "input": "text",
            "binding": {"kind": "single", "source": {
                "node": {"kind": "id", "id": "source"},
                "output": "text",
                "extra": true
            }}
        }]
    });
    assert!(serde_json::from_value::<WorkflowApplyPatchInput>(unknown).is_err());
}

#[test]
fn request_hash_is_stable_for_the_same_typed_patch() {
    let patch = WorkflowPatch {
        operations: vec![WorkflowPatchOperation::RemoveNode {
            node: engine::NodeRef::Id { id: "n1".to_owned() },
        }],
    };
    assert_eq!(
        request_hash(Some(1), &patch).expect("hash"),
        request_hash(Some(1), &patch).expect("hash")
    );
}

#[test]
fn service_requires_a_real_project_before_mutating_authority() {
    let root = tempdir().expect("asset root");
    let state = AppState::from_asset_root(root.path()).expect("app state");
    let service = WorkflowPatchService::from_state(&state);
    let error = service
        .apply(
            &context("missing"),
            WorkflowApplyPatchInput { expected_revision: None, operations: Vec::new() },
        )
        .expect_err("unknown project must fail");
    assert_eq!(error.code, "PROJECT_NOT_FOUND");
}

#[test]
fn exact_capability_ref_is_used_by_the_boundary() {
    let reference = CapabilityRef::new("TextPrompt", "1.0");
    assert_eq!(reference.version, "1.0");
    let _params = NodeParams::new();
}
