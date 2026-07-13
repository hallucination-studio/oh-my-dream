use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AsyncInput {
    value: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AsyncOutput {
    value: String,
}

#[tokio::test]
async fn assistant_operation_dispatches_async_handler_without_blocking_bridge() {
    let registration = OperationRegistration::new::<AsyncInput, AsyncOutput, _>(
        "workspace.async",
        1,
        "Execute an async handler.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |context: &RequestContext, input: AsyncInput| {
            let request_id = context.request_id().to_owned();
            async move { Ok(AsyncOutput { value: format!("{}:{request_id}", input.value) }) }
        },
    )
    .expect("async registration should be valid");

    let output = registration
        .dispatch(
            &RequestContext::new("project", "session", "request", 1, None),
            json!({
                "value": "input"
            }),
        )
        .await
        .expect("async dispatch should succeed");

    assert_eq!(output, json!({ "value": "input:request" }));
}
