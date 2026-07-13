use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct PrimitiveRootInput;

impl JsonSchema for PrimitiveRootInput {
    fn schema_name() -> String {
        "PrimitiveRootInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schema(json!({ "type": "string" }))
    }
}

#[derive(Debug, Deserialize)]
struct ArrayRootInput;

impl JsonSchema for ArrayRootInput {
    fn schema_name() -> String {
        "ArrayRootInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schema(json!({ "type": "array", "items": { "type": "string" } }))
    }
}

#[derive(Debug, Deserialize)]
struct RequiredOnlyNestedInput;

impl JsonSchema for RequiredOnlyNestedInput {
    fn schema_name() -> String {
        "RequiredOnlyNestedInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({ "required": ["nested"] }))
    }
}

#[derive(Debug, Deserialize)]
struct PatternPropertiesOnlyNestedInput;

impl JsonSchema for PatternPropertiesOnlyNestedInput {
    fn schema_name() -> String {
        "PatternPropertiesOnlyNestedInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({ "patternProperties": { "^x-": { "type": "string" } } }))
    }
}

#[derive(Debug, Deserialize)]
struct NotFalseInput;

impl JsonSchema for NotFalseInput {
    fn schema_name() -> String {
        "NotFalseInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({ "not": false }))
    }
}

#[derive(Debug, Deserialize)]
struct IfFalseThenInput;

impl JsonSchema for IfFalseThenInput {
    fn schema_name() -> String {
        "IfFalseThenInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({ "if": false, "then": { "type": "string" } }))
    }
}

#[derive(Debug, Deserialize)]
struct AdditionalItemsInput;

impl JsonSchema for AdditionalItemsInput {
    fn schema_name() -> String {
        "AdditionalItemsInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({
            "type": "array",
            "items": [{ "type": "string" }],
            "additionalItems": false
        }))
    }
}

#[derive(Debug, Deserialize)]
struct TupleItemsInput;

impl JsonSchema for TupleItemsInput {
    fn schema_name() -> String {
        "TupleItemsInput".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_root(json!({ "type": "array", "items": [] }))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[test]
fn assistant_operation_requires_object_input_root_in_both_modes() {
    let strict = registration_error::<PrimitiveRootInput>(OperationInputSchemaMode::Strict);
    let patch =
        registration_error::<ArrayRootInput>(OperationInputSchemaMode::WorkflowPatchParamsOpen);

    for error in [strict, patch] {
        assert_eq!(
            error,
            OperationRegistrationError::InvalidInputRootSchema { schema_path: "#".to_owned() }
        );
    }
}

#[test]
fn assistant_operation_treats_all_object_keywords_as_object_evidence() {
    let required = registration_error::<RequiredOnlyNestedInput>(OperationInputSchemaMode::Strict);
    assert_eq!(
        required,
        OperationRegistrationError::OpenObjectSchema {
            schema_kind: "input",
            schema_path: "#/properties/value".to_owned(),
        }
    );

    let pattern =
        registration_error::<PatternPropertiesOnlyNestedInput>(OperationInputSchemaMode::Strict);
    assert_unsupported(pattern, "#/properties/value", "patternProperties");
}

#[test]
fn assistant_operation_rejects_unsupported_draft7_semantics() {
    for (error, keyword) in [
        (registration_error::<NotFalseInput>(OperationInputSchemaMode::Strict), "not"),
        (registration_error::<IfFalseThenInput>(OperationInputSchemaMode::Strict), "if"),
        (
            registration_error::<AdditionalItemsInput>(OperationInputSchemaMode::Strict),
            "additionalItems",
        ),
        (registration_error::<TupleItemsInput>(OperationInputSchemaMode::Strict), "items"),
    ] {
        assert_unsupported(error, "#/properties/value", keyword);
    }
}

fn registration_error<I>(mode: OperationInputSchemaMode) -> OperationRegistrationError
where
    I: DeserializeOwned + JsonSchema + Send + 'static,
{
    OperationRegistration::new::<I, StrictOutput, _>(
        "workspace.supported_subset",
        1,
        "Reject schema outside the supported registration subset.",
        OperationEffect::LocalRead,
        mode,
        |_context: &RequestContext, _input: I| async move {
            Ok(StrictOutput { result: String::new() })
        },
    )
    .err()
    .expect("custom schema should be rejected")
}

fn closed_root(property_schema: Value) -> Schema {
    schema(json!({
        "type": "object",
        "required": ["value"],
        "properties": { "value": property_schema },
        "additionalProperties": false
    }))
}

fn assert_unsupported(error: OperationRegistrationError, path: &str, keyword: &str) {
    assert_eq!(
        error,
        OperationRegistrationError::UnsupportedSchemaKeyword {
            schema_kind: "input",
            schema_path: path.to_owned(),
            keyword: keyword.to_owned(),
        }
    );
}

fn schema(value: Value) -> Schema {
    serde_json::from_value(value).expect("test schema should deserialize")
}
