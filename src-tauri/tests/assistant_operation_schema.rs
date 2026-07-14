use std::collections::BTreeMap;

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::{
    JsonSchema,
    r#gen::SchemaGenerator,
    schema::{Schema, SchemaObject},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictInput {
    query: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PatchInput {
    expected_revision: u64,
    params: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PatchOutput {
    revision: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct OpenRootInput {
    query: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct OpenRootOutput {
    result: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct OpenNested {
    value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClosedInputWithOpenNested {
    nested: OpenNested,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClosedOutputWithOpenNested {
    nested: OpenNested,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PatchWithOpenNestedInput {
    params: BTreeMap<String, Value>,
    nested: OpenNested,
}

#[derive(Debug, Deserialize)]
struct EmptySchemaInput;

impl JsonSchema for EmptySchemaInput {
    fn schema_name() -> String {
        "EmptySchemaInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject::default())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClosedRecursiveInput {
    label: String,
    next: Box<ClosedRecursiveInput>,
}

#[test]
fn assistant_operation_strict_mode_generates_fully_closed_schemas() {
    let registration = strict_registration();

    assert_eq!(registration.input_schema_mode(), OperationInputSchemaMode::Strict);
    assert!(registration.sdk_strict_json_schema());
    assert_eq!(registration.input_schema()["additionalProperties"], json!(false));
    assert_eq!(registration.output_schema()["additionalProperties"], json!(false));
}

#[test]
fn assistant_operation_patch_mode_opens_only_canonical_params_payload() {
    let registration = patch_registration();
    let schema = registration.input_schema();

    assert_eq!(registration.input_schema_mode(), OperationInputSchemaMode::WorkflowPatchParamsOpen);
    assert!(!registration.sdk_strict_json_schema());
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["properties"]["params"]["additionalProperties"], json!(true));
}

#[test]
fn assistant_operation_rejects_open_input_and_output_roots() {
    let input_error = OperationRegistration::new::<OpenRootInput, StrictOutput, _>(
        "workspace.open_input",
        1,
        "Reject open input.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: OpenRootInput| async move {
            Ok(StrictOutput { result: input.query })
        },
    )
    .err()
    .expect("open input root must be rejected");
    let output_error = OperationRegistration::new::<StrictInput, OpenRootOutput, _>(
        "workspace.open_output",
        1,
        "Reject open output.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: StrictInput| async move {
            Ok(OpenRootOutput { result: input.query })
        },
    )
    .err()
    .expect("open output root must be rejected");

    assert_open_error(input_error, "input", "#");
    assert_open_error(output_error, "output", "#");
}

#[test]
fn assistant_operation_rejects_open_nested_fixed_input_and_output() {
    let input_error = OperationRegistration::new::<ClosedInputWithOpenNested, StrictOutput, _>(
        "workspace.open_nested_input",
        1,
        "Reject open nested input.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: ClosedInputWithOpenNested| async move {
            Ok(StrictOutput { result: input.nested.value })
        },
    )
    .err()
    .expect("open nested input must be rejected");
    let output_error = OperationRegistration::new::<PatchInput, ClosedOutputWithOpenNested, _>(
        "workspace.open_nested_output",
        1,
        "Reject open nested output.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: PatchInput| async move {
            Ok(ClosedOutputWithOpenNested {
                nested: OpenNested { value: input.expected_revision.to_string() },
            })
        },
    )
    .err()
    .expect("open nested output must be rejected");

    assert_open_error(input_error, "input", "#/properties/nested");
    assert_open_error(output_error, "output", "#/properties/nested");
}

#[test]
fn assistant_operation_patch_mode_rejects_openness_outside_params() {
    let error = OperationRegistration::new::<PatchWithOpenNestedInput, PatchOutput, _>(
        "workflow.invalid_patch",
        1,
        "Reject non-params openness.",
        OperationEffect::VisibleReversibleWorkflowPatch,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: PatchWithOpenNestedInput| async move {
            let _params = input.params;
            Ok(PatchOutput { revision: input.nested.value.len() as u64 })
        },
    )
    .err()
    .expect("patch mode must reject another open object");

    assert_open_error(error, "input", "#/properties/nested");
}

#[test]
fn assistant_operation_requires_an_object_root_schema() {
    let error = OperationRegistration::new::<EmptySchemaInput, StrictOutput, _>(
        "workspace.empty_schema",
        1,
        "Reject empty schema.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, _input: EmptySchemaInput| async move {
            Ok(StrictOutput { result: String::new() })
        },
    )
    .err()
    .expect("empty schema must be rejected");

    assert_eq!(
        error,
        OperationRegistrationError::InvalidInputRootSchema { schema_path: "#".to_owned() }
    );
}

#[test]
fn assistant_operation_closed_recursive_schema_terminates() {
    let registration = OperationRegistration::new::<ClosedRecursiveInput, StrictOutput, _>(
        "workspace.recursive",
        1,
        "Accept a closed recursive schema.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: ClosedRecursiveInput| async move {
            let _next = input.next;
            Ok(StrictOutput { result: input.label })
        },
    )
    .expect("closed recursive schema should register");

    assert!(registration.input_schema().to_string().contains("\"$ref\""));
}

fn strict_registration() -> OperationRegistration {
    OperationRegistration::new::<StrictInput, StrictOutput, _>(
        "workspace.strict",
        1,
        "Strict operation.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: StrictInput| async move {
            Ok(StrictOutput { result: input.query })
        },
    )
    .expect("strict registration should be valid")
}

fn patch_registration() -> OperationRegistration {
    OperationRegistration::new::<PatchInput, PatchOutput, _>(
        "workflow.apply_patch",
        1,
        "Patch operation.",
        OperationEffect::VisibleReversibleWorkflowPatch,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: PatchInput| async move {
            let _params = input.params;
            Ok(PatchOutput { revision: input.expected_revision + 1 })
        },
    )
    .expect("patch registration should be valid")
}

fn assert_open_error(error: OperationRegistrationError, schema_kind: &'static str, path: &str) {
    assert_eq!(
        error,
        OperationRegistrationError::OpenObjectSchema { schema_kind, schema_path: path.to_owned() }
    );
}
