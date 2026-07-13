use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

#[derive(Debug, Deserialize)]
struct TypeArrayObjectInput;

impl JsonSchema for TypeArrayObjectInput {
    fn schema_name() -> String {
        "TypeArrayObjectInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schema(json!({ "type": ["object", "null"] }))
    }
}

#[derive(Debug, Deserialize)]
struct StandaloneIfInput;

impl JsonSchema for StandaloneIfInput {
    fn schema_name() -> String {
        "StandaloneIfInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schema(json!({ "if": { "type": "string" } }))
    }
}

#[derive(Debug, Deserialize)]
struct StandaloneBranchesInput;

impl JsonSchema for StandaloneBranchesInput {
    fn schema_name() -> String {
        "StandaloneBranchesInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schema(json!({ "then": { "type": "string" }, "else": { "type": "number" } }))
    }
}

#[derive(Debug, Deserialize)]
struct EmptyAllOfInput;

impl JsonSchema for EmptyAllOfInput {
    fn schema_name() -> String {
        "EmptyAllOfInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schema(json!({ "allOf": [] }))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[test]
fn assistant_operation_recognizes_object_inside_type_array() {
    assert_eq!(
        registration_error::<TypeArrayObjectInput>(),
        OperationRegistrationError::InvalidInputRootSchema { schema_path: "#".to_owned() }
    );
}

#[test]
fn assistant_operation_rejects_non_constraining_conditionals_and_empty_all_of() {
    assert_unsupported(registration_error::<StandaloneIfInput>(), "if");
    assert_unsupported(registration_error::<StandaloneBranchesInput>(), "then");
    assert_eq!(
        registration_error::<EmptyAllOfInput>(),
        OperationRegistrationError::UnconstrainedSchema {
            schema_kind: "input",
            schema_path: "#".to_owned(),
        }
    );
}

fn assert_unsupported(error: OperationRegistrationError, keyword: &str) {
    assert_eq!(
        error,
        OperationRegistrationError::UnsupportedSchemaKeyword {
            schema_kind: "input",
            schema_path: "#".to_owned(),
            keyword: keyword.to_owned(),
        }
    );
}

fn registration_error<I>() -> OperationRegistrationError
where
    I: DeserializeOwned + JsonSchema + Send + 'static,
{
    OperationRegistration::new::<I, StrictOutput, _>(
        "workspace.custom_schema",
        1,
        "Reject unsupported custom schema.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, _input: I| async move {
            Ok(StrictOutput { result: String::new() })
        },
    )
    .err()
    .expect("custom schema should be rejected")
}

fn schema(value: serde_json::Value) -> Schema {
    serde_json::from_value(value).expect("test schema should deserialize")
}
