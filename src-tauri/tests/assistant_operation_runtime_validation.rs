use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use oh_my_dream_tauri::assistant_operations::{
    OperationDispatchError, OperationEffect, OperationInputSchemaMode, OperationRegistration,
    RequestContext,
};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct CanonicalNestedSchema {
    allowed: String,
}

#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct CanonicalInputSchema {
    nested: CanonicalNestedSchema,
}

#[derive(Debug, Deserialize)]
struct PermissiveInput {
    nested: Value,
}

impl JsonSchema for PermissiveInput {
    fn schema_name() -> String {
        "PermissiveInput".to_owned()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        CanonicalInputSchema::json_schema(generator)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    result: String,
}

#[tokio::test]
async fn assistant_operation_validates_canonical_schema_before_deserialization() {
    let executions = Arc::new(AtomicUsize::new(0));
    let handler_executions = Arc::clone(&executions);
    let registration = OperationRegistration::new::<PermissiveInput, StrictOutput, _>(
        "workspace.schema_guard",
        1,
        "Validate canonical model input.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        move |_context: &RequestContext, input: PermissiveInput| {
            let executions = Arc::clone(&handler_executions);
            async move {
                executions.fetch_add(1, Ordering::SeqCst);
                Ok(StrictOutput { result: input.nested.to_string() })
            }
        },
    )
    .expect("mismatched registration should be valid");

    let error = registration
        .dispatch(
            &RequestContext::new("project", "session", "request", 1, None),
            json!({ "nested": { "allowed": "yes", "unexpected": true } }),
        )
        .await
        .expect_err("canonical schema must reject nested unknown fields");

    let OperationDispatchError::SchemaValidation { operation_id, violations } = error else {
        panic!("expected structured schema validation error");
    };
    assert_eq!(operation_id, "workspace.schema_guard");
    assert!(!violations.is_empty());
    assert!(violations.iter().any(|violation| violation.instance_path == "/nested"));
    assert!(violations.iter().all(|violation| !violation.message.is_empty()));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}
