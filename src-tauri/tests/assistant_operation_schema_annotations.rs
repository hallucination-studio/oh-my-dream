use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, OperationRegistrationError,
    RequestContext,
};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct AnnotatedSchemaShape {
    query: String,
}

#[derive(Debug, Deserialize)]
struct AnnotatedInput {
    query: String,
}

impl JsonSchema for AnnotatedInput {
    fn schema_name() -> String {
        "AnnotatedInput".to_owned()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        annotated_schema(generator, false)
    }
}

#[derive(Debug, Deserialize)]
struct AnnotatedInputWithOpenSubschema {
    query: String,
}

impl JsonSchema for AnnotatedInputWithOpenSubschema {
    fn schema_name() -> String {
        "AnnotatedInputWithOpenSubschema".to_owned()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        annotated_schema(generator, true)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[test]
fn assistant_operation_ignores_annotations_but_validates_real_subschemas() {
    let registration = OperationRegistration::new::<AnnotatedInput, StrictOutput, _>(
        "workspace.annotated",
        1,
        "Accept instance-valued annotations.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: AnnotatedInput| async move {
            Ok(StrictOutput { result: input.query })
        },
    )
    .expect("object-valued annotations are not schemas");

    assert_eq!(registration.input_schema()["default"], json!({ "query": {} }));
    let error = OperationRegistration::new::<AnnotatedInputWithOpenSubschema, StrictOutput, _>(
        "workspace.annotated_open",
        1,
        "Reject a real unconstrained subschema.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, input: AnnotatedInputWithOpenSubschema| async move {
            Ok(StrictOutput { result: input.query })
        },
    )
    .err()
    .expect("allOf true must remain subject to validation");
    assert_eq!(
        error,
        OperationRegistrationError::UnconstrainedSchema {
            schema_kind: "input",
            schema_path: "#/allOf/0".to_owned(),
        }
    );
}

fn annotated_schema(generator: &mut SchemaGenerator, open_subschema: bool) -> Schema {
    let mut schema = AnnotatedSchemaShape::json_schema(generator).into_object();
    {
        let metadata = schema.metadata();
        metadata.default = Some(json!({ "query": {} }));
        metadata.examples.push(json!({ "query": { "example": true } }));
    }
    if open_subschema {
        schema.subschemas().all_of = Some(vec![Schema::Bool(true)]);
    }
    Schema::Object(schema)
}
