use std::collections::BTreeMap;

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MissingRequiredInput {
    optional: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MissingRequiredOutput {
    optional: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiredNullableInput {
    optional: Option<String>,
}

impl JsonSchema for RequiredNullableInput {
    fn schema_name() -> String {
        "RequiredNullableInput".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        serde_json::from_value(json!({
            "type": "object",
            "required": ["optional"],
            "properties": {
                "optional": { "type": ["string", "null"] }
            },
            "additionalProperties": false
        }))
        .expect("test schema must deserialize")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NestedInput {
    nested: NestedMissingRequired,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NestedMissingRequired {
    optional: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PatchOptionalInput {
    params: BTreeMap<String, Value>,
    optional: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[test]
fn assistant_operation_strict_mode_rejects_missing_required_input_and_output_properties() {
    let input_error = OperationRegistration::new::<MissingRequiredInput, StrictOutput, _>(
        "workspace.optional_input",
        1,
        "Reject optional input property.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: MissingRequiredInput| async move {
            Ok(StrictOutput { result: input.optional.unwrap_or_default() })
        },
    )
    .err()
    .expect("strict input properties must be required");
    let output_error =
        OperationRegistration::new::<RequiredNullableInput, MissingRequiredOutput, _>(
            "workspace.optional_output",
            1,
            "Reject optional output property.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            |_context: &RequestContext, input: RequiredNullableInput| async move {
                Ok(MissingRequiredOutput { optional: input.optional })
            },
        )
        .err()
        .expect("strict output properties must be required");

    assert_missing_required(input_error, "input", "#");
    assert_missing_required(output_error, "output", "#");
}

#[test]
fn assistant_operation_strict_mode_checks_required_properties_through_refs() {
    let error = OperationRegistration::new::<NestedInput, StrictOutput, _>(
        "workspace.nested_optional",
        1,
        "Reject nested optional property.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: NestedInput| async move {
            Ok(StrictOutput { result: input.nested.optional.unwrap_or_default() })
        },
    )
    .err()
    .expect("nested strict properties must be required");

    assert_missing_required(error, "input", "#/properties/nested");
}

#[tokio::test]
async fn assistant_operation_required_nullable_is_strict_but_patch_mode_is_not() {
    let strict = OperationRegistration::new::<RequiredNullableInput, StrictOutput, _>(
        "workspace.required_nullable",
        1,
        "Accept required nullable input.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: RequiredNullableInput| async move {
            Ok(StrictOutput { result: input.optional.unwrap_or_default() })
        },
    )
    .expect("required nullable input should be strict");
    let patch = OperationRegistration::new::<PatchOptionalInput, StrictOutput, _>(
        "workflow.optional_patch",
        1,
        "Patch input does not require strict-required semantics.",
        OperationEffect::VisibleReversibleWorkflowPatch,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: PatchOptionalInput| async move {
            let _params = input.params;
            Ok(StrictOutput { result: input.optional.unwrap_or_default() })
        },
    )
    .expect("patch mode should allow optional non-params fields");

    assert!(strict.sdk_strict_json_schema());
    assert!(!patch.sdk_strict_json_schema());
    let output = strict
        .dispatch(
            &RequestContext::new("project", "session", "request", 1, None),
            json!({ "optional": null }),
        )
        .await
        .expect("required nullable input should accept null");
    assert_eq!(output, json!({ "result": "" }));
}

fn assert_missing_required(
    error: OperationRegistrationError,
    schema_kind: &'static str,
    object_path: &str,
) {
    assert_eq!(
        error,
        OperationRegistrationError::MissingRequiredProperty {
            schema_kind,
            object_path: object_path.to_owned(),
            property: "optional".to_owned(),
        }
    );
}
