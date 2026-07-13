use std::sync::{Arc, Mutex};

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationRegistration,
    RequestContext,
};
use oh_my_dream_tauri::assistant_runtime::{
    AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome, TrustedInvocationContext,
};
use tempfile::TempDir;

use super::common::{LocalReadInput, LocalReadOutput, python_fixture_command};

#[tokio::test]
async fn assistant_runtime_streams_tool_request_through_rust_and_completes() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("session.sqlite3");
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let registration = local_read_registration(Arc::clone(&contexts));
    let runtime = AssistantRuntime::new(python_fixture_command("tool"), vec![registration])
        .expect("runtime registrations should be valid");

    let outcome = runtime
        .invoke(
            AssistantInvocation::new(
                "invoke-1",
                "session-1",
                &session_path,
                Some("Use the operation.".to_owned()),
            ),
            TrustedInvocationContext::new("project-7", "request-9"),
        )
        .await
        .expect("assistant invocation should complete");

    let AssistantRuntimeOutcome::Completed(completed) = outcome else {
        panic!("expected completed runtime outcome");
    };
    assert_eq!(completed.final_output(), &serde_json::json!("tool completed"));
    assert_eq!(completed.messages(), ["tool completed"]);
    assert_eq!(completed.snapshot().session_id(), "session-1");
    assert_eq!(completed.snapshot().status(), "completed");
    assert_eq!(completed.snapshot().state(), &serde_json::Value::Null);
    assert_eq!(completed.operation_calls().len(), 1);
    assert_eq!(completed.operation_calls()[0].arguments_json(), r#"{  "query" : "current" }"#);
    assert_eq!(
        completed.operation_calls()[0].output_json(),
        r#"{"project_id":"project-7","request_id":"request-9","result":"current"}"#
    );
    assert_eq!(
        contexts.lock().expect("contexts lock should remain available").as_slice(),
        [("project-7".to_owned(), "session-1".to_owned(), "request-9".to_owned())]
    );
    assert!(session_path.exists());
}

fn local_read_registration(
    contexts: Arc<Mutex<Vec<(String, String, String)>>>,
) -> OperationRegistration {
    OperationRegistration::new::<LocalReadInput, LocalReadOutput, _>(
        "workspace_get_snapshot",
        3,
        "Read deterministic local state.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: LocalReadInput| {
            let observed_contexts = Arc::clone(&contexts);
            let project_id = context.project_id().to_owned();
            let request_id = context.request_id().to_owned();
            let session_id = context.session_id().to_owned();
            async move {
                observed_contexts
                    .lock()
                    .map_err(|error| OperationHandlerError::new("LOCK_FAILED", error.to_string()))?
                    .push((project_id.clone(), session_id, request_id.clone()));
                Ok(LocalReadOutput { project_id, request_id, result: input.query })
            }
        },
    )
    .expect("test registration should be valid")
}
