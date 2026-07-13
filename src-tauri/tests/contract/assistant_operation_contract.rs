use std::collections::BTreeMap;

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const OPERATION_CONTRACT_KEYS: [&str; 8] = [
    "description",
    "effect",
    "id",
    "input_schema",
    "needs_approval",
    "output_schema",
    "strict_json_schema",
    "version",
];

const TRUSTED_ARGUMENT_NAMES: [&str; 5] =
    ["project_id", "session_id", "request_id", "tool_version", "approved_effect"];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadInput {
    query: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PatchInput {
    expected_revision: u64,
    params: BTreeMap<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreparedInput {
    proposal_id: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Output {
    result: String,
}

pub(super) fn fixture() -> Value {
    let read =
        OperationRegistration::new::<ReadInput, Output, _>(
            "workspace_get_snapshot",
            1,
            "Read local workspace state.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            |_context: &RequestContext, input: ReadInput| async move {
                Ok(Output { result: input.query })
            },
        )
        .expect("register local read contract");
    let patch = OperationRegistration::new::<PatchInput, Output, _>(
        "workflow_apply_patch",
        2,
        "Apply a visible reversible Workflow patch.",
        OperationEffect::VisibleReversibleWorkflowPatch,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: PatchInput| async move {
            Ok(Output { result: format!("{}:{}", input.expected_revision, input.params.len()) })
        },
    )
    .expect("register Workflow patch contract");
    let prepared = OperationRegistration::new::<PreparedInput, Output, _>(
        "proposal_execute",
        3,
        "Execute a previously prepared proposal.",
        OperationEffect::PreparedApprovalExecution,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: PreparedInput| async move {
            Ok(Output { result: input.proposal_id })
        },
    )
    .expect("register prepared execution contract");
    json!({ "operations": [read.contract(), patch.contract(), prepared.contract()] })
}

pub(super) fn assert_fixture(fixture: &Value) {
    let operations = fixture["operations"].as_array().expect("operations array");
    assert_eq!(operations.len(), 3);
    assert_operation(&operations[0], "workspace_get_snapshot", "local_read", true, false);
    assert_operation(
        &operations[1],
        "workflow_apply_patch",
        "visible_reversible_workflow_patch",
        false,
        false,
    );
    assert_operation(&operations[2], "proposal_execute", "prepared_approval_execution", true, true);
    assert_eq!(operations[0]["input_schema"]["required"], json!(["query"]));
    assert_eq!(operations[1]["input_schema"]["required"], json!(["expected_revision", "params"]));
    assert_eq!(operations[2]["input_schema"]["required"], json!(["proposal_id"]));
    assert_eq!(operations[1]["input_schema"]["properties"]["params"]["additionalProperties"], true);
}

fn assert_operation(operation: &Value, id: &str, effect: &str, strict: bool, approval: bool) {
    let object = operation.as_object().expect("serialized operation contract");
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, OPERATION_CONTRACT_KEYS);
    assert_eq!(operation["id"], id);
    assert_eq!(operation["effect"], effect);
    assert_eq!(operation["strict_json_schema"], strict);
    assert_eq!(operation["needs_approval"], approval);
    assert_no_trusted_schema_fields(&operation["input_schema"])
        .expect("trusted context must not be exposed as model arguments");
}

fn assert_no_trusted_schema_fields(schema: &Value) -> Result<(), String> {
    inspect_schema(schema, "#")
}

fn inspect_schema(schema: &Value, path: &str) -> Result<(), String> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for name in properties.keys() {
            if is_trusted_argument(name) {
                return Err(format!("trusted property {name} at {path}"));
            }
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if is_trusted_argument(name) {
                return Err(format!("trusted required field {name} at {path}"));
            }
        }
    }
    for keyword in [
        "properties",
        "patternProperties",
        "definitions",
        "$defs",
        "dependentSchemas",
        "dependencies",
    ] {
        inspect_schema_map(object.get(keyword), path, keyword)?;
    }
    for keyword in [
        "additionalProperties",
        "additionalItems",
        "contains",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
        "unevaluatedProperties",
        "unevaluatedItems",
    ] {
        if let Some(subschema) = object.get(keyword) {
            inspect_schema(subschema, &format!("{path}/{keyword}"))?;
        }
    }
    for keyword in ["items", "prefixItems", "allOf", "anyOf", "oneOf"] {
        if let Some(subschema) = object.get(keyword) {
            inspect_schema_or_array(subschema, path, keyword)?;
        }
    }
    Ok(())
}

fn inspect_schema_map(value: Option<&Value>, path: &str, keyword: &str) -> Result<(), String> {
    let Some(entries) = value.and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, subschema) in entries {
        inspect_schema(subschema, &format!("{path}/{keyword}/{}", escape_pointer(name)))?;
    }
    Ok(())
}

fn inspect_schema_or_array(value: &Value, path: &str, keyword: &str) -> Result<(), String> {
    if let Some(subschemas) = value.as_array() {
        for (index, subschema) in subschemas.iter().enumerate() {
            inspect_schema(subschema, &format!("{path}/{keyword}/{index}"))?;
        }
        return Ok(());
    }
    inspect_schema(value, &format!("{path}/{keyword}"))
}

fn is_trusted_argument(name: &str) -> bool {
    TRUSTED_ARGUMENT_NAMES.contains(&name)
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_words_in_schema_annotations_are_ignored() {
        let schema = json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "default": {
                "project_id": "project-default",
                "session_id": "session-default",
                "request_id": "request-default"
            },
            "examples": [{
                "tool_version": 7,
                "approved_effect": "example approval text"
            }]
        });

        assert_no_trusted_schema_fields(&schema).expect("annotations are not model fields");
    }

    #[test]
    fn nested_trusted_property_is_rejected() {
        let schema = json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": { "session_id": { "type": "string" } },
                    "required": ["session_id"]
                }
            }
        });

        let error = assert_no_trusted_schema_fields(&schema)
            .expect_err("nested trusted model field must be rejected");
        assert_eq!(error, "trusted property session_id at #/properties/nested");
    }

    #[test]
    fn trusted_required_entry_is_rejected() {
        let schema = json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query", "request_id"]
        });

        let error = assert_no_trusted_schema_fields(&schema)
            .expect_err("trusted required entry must be rejected");
        assert_eq!(error, "trusted required field request_id at #");
    }
}
