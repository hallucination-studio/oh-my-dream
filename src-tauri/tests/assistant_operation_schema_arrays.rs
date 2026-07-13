use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
struct InputWithUnconstrainedItems {
    values: Vec<Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OutputWithUnconstrainedItems {
    values: Vec<Value>,
}

#[test]
fn assistant_operation_rejects_boolean_true_items_in_input_and_output() {
    let input_error = OperationRegistration::new::<InputWithUnconstrainedItems, StrictOutput, _>(
        "workspace.array_input",
        1,
        "Reject unconstrained input items.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: InputWithUnconstrainedItems| async move {
            Ok(StrictOutput { result: input.values.len().to_string() })
        },
    )
    .err()
    .expect("input items true must be rejected");
    let output_error = OperationRegistration::new::<StrictInput, OutputWithUnconstrainedItems, _>(
        "workspace.array_output",
        1,
        "Reject unconstrained output items.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: StrictInput| async move {
            Ok(OutputWithUnconstrainedItems { values: vec![Value::String(input.query)] })
        },
    )
    .err()
    .expect("output items true must be rejected");

    assert_unconstrained_items(input_error, "input");
    assert_unconstrained_items(output_error, "output");
}

fn assert_unconstrained_items(error: OperationRegistrationError, schema_kind: &'static str) {
    assert_eq!(
        error,
        OperationRegistrationError::UnconstrainedSchema {
            schema_kind,
            schema_path: "#/properties/values/items".to_owned(),
        }
    );
}
