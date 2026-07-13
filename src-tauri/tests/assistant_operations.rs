use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use oh_my_dream_tauri::assistant_operations::{
    ApprovedEffect, OperationDispatchError, OperationEffect, OperationHandlerError,
    OperationInputSchemaMode, OperationRegistration, RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LocalReadInput {
    query: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LocalReadOutput {
    project_id: String,
    result: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkflowPatchInput {
    expected_revision: u64,
    params: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkflowPatchOutput {
    revision: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreparedExecutionInput {
    proposal_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreparedExecutionOutput {
    approval_id: String,
}

#[tokio::test]
async fn assistant_operation_dispatches_typed_input_with_trusted_context() {
    let registration = local_read_registration();
    let context = request_context(None);

    let output = registration
        .dispatch(&context, json!({ "query": "current workflow" }))
        .await
        .expect("registered handler should execute");

    assert_eq!(
        output,
        json!({
            "project_id": "project-7",
            "result": "current workflow for session-3"
        })
    );
}

#[tokio::test]
async fn assistant_operation_rejects_unknown_fixed_input_fields_before_dispatch() {
    let registration = local_read_registration();
    let error = registration
        .dispatch(
            &request_context(None),
            json!({ "query": "workflow", "project_id": "model-controlled" }),
        )
        .await
        .expect_err("fixed input must reject unknown fields");

    assert!(matches!(
        error,
        OperationDispatchError::SchemaValidation { operation_id, .. }
            if operation_id == "workspace.local_read"
    ));
}

#[tokio::test]
async fn assistant_operation_rejects_tool_version_mismatch_before_dispatch() {
    let executions = Arc::new(AtomicUsize::new(0));
    let handler_executions = Arc::clone(&executions);
    let registration = OperationRegistration::new::<LocalReadInput, LocalReadOutput, _>(
        "workspace.local_read",
        1,
        "Read local workspace state.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: LocalReadInput| {
            let executions = Arc::clone(&handler_executions);
            let project_id = context.project_id().to_owned();
            async move {
                executions.fetch_add(1, Ordering::SeqCst);
                Ok(LocalReadOutput { project_id, result: input.query })
            }
        },
    )
    .expect("local read registration should be valid");

    let error = registration
        .dispatch(&request_context_with_version(2, None), json!({ "query": "workflow" }))
        .await
        .expect_err("mismatched tool version must be rejected");

    assert_eq!(
        error,
        OperationDispatchError::ToolVersionMismatch {
            operation_id: "workspace.local_read".to_owned(),
            registered_version: 1,
            context_version: 2,
        }
    );
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_operation_requires_prepared_approval_before_dispatch() {
    let (registration, executions) = counting_prepared_execution_registration();

    let error = registration
        .dispatch(&request_context(None), json!({ "proposal_id": "proposal-4" }))
        .await
        .expect_err("prepared execution must require approval");

    assert_eq!(
        error,
        OperationDispatchError::ApprovalRequired {
            operation_id: "proposal.execute".to_owned(),
            operation_version: 1,
        }
    );
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_operation_rejects_wrong_prepared_approval_before_dispatch() {
    let wrong_approvals = [
        ApprovedEffect::new("different.operation", 1, "approval-wrong-id"),
        ApprovedEffect::new("proposal.execute", 2, "approval-wrong-version"),
    ];

    for approved_effect in wrong_approvals {
        let (registration, executions) = counting_prepared_execution_registration();
        let approved_operation_id = approved_effect.operation_id().to_owned();
        let approved_operation_version = approved_effect.operation_version();

        let error = registration
            .dispatch(
                &request_context(Some(approved_effect)),
                json!({ "proposal_id": "proposal-4" }),
            )
            .await
            .expect_err("approval must match the exact operation contract");

        assert_eq!(
            error,
            OperationDispatchError::ApprovalMismatch {
                operation_id: "proposal.execute".to_owned(),
                operation_version: 1,
                approved_operation_id,
                approved_operation_version,
            }
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn assistant_operation_exposes_stable_metadata_and_effects() {
    let local_read = local_read_registration();
    let workflow_patch = workflow_patch_registration();
    let prepared_execution = prepared_execution_registration();

    assert_eq!(local_read.id(), "workspace.local_read");
    assert_eq!(local_read.version(), 1);
    assert_eq!(local_read.description(), "Read local workspace state.");
    assert_eq!(local_read.effect(), OperationEffect::LocalRead);
    assert_eq!(workflow_patch.effect(), OperationEffect::VisibleReversibleWorkflowPatch);
    assert_eq!(prepared_execution.effect(), OperationEffect::PreparedApprovalExecution);
}

#[tokio::test]
async fn assistant_operation_keeps_request_context_out_of_model_schema() {
    let registration = prepared_execution_registration();
    let schema = registration.input_schema().to_string();
    let context = request_context(Some(ApprovedEffect::new("proposal.execute", 1, "approval-9")));

    assert!(!schema.contains("project_id"));
    assert!(!schema.contains("session_id"));
    assert!(!schema.contains("request_id"));
    assert!(!schema.contains("tool_version"));
    assert!(!schema.contains("approved_effect"));

    let output = registration
        .dispatch(&context, json!({ "proposal_id": "proposal-4" }))
        .await
        .expect("trusted approval should reach the handler separately");
    assert_eq!(output, json!({ "approval_id": "approval-9" }));
}

fn local_read_registration() -> OperationRegistration {
    OperationRegistration::new::<LocalReadInput, LocalReadOutput, _>(
        "workspace.local_read",
        1,
        "Read local workspace state.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |context: &RequestContext, input: LocalReadInput| {
            let project_id = context.project_id().to_owned();
            let session_id = context.session_id().to_owned();
            async move {
                Ok(LocalReadOutput {
                    project_id,
                    result: format!("{} for {session_id}", input.query),
                })
            }
        },
    )
    .expect("local read registration should be valid")
}

fn workflow_patch_registration() -> OperationRegistration {
    OperationRegistration::new::<WorkflowPatchInput, WorkflowPatchOutput, _>(
        "workflow.apply_patch",
        1,
        "Apply a visible reversible Workflow patch.",
        OperationEffect::VisibleReversibleWorkflowPatch,
        OperationInputSchemaMode::WorkflowPatchParamsOpen,
        |_context: &RequestContext, input: WorkflowPatchInput| async move {
            let _params = input.params;
            Ok(WorkflowPatchOutput { revision: input.expected_revision + 1 })
        },
    )
    .expect("workflow patch registration should be valid")
}

fn prepared_execution_registration() -> OperationRegistration {
    OperationRegistration::new::<PreparedExecutionInput, PreparedExecutionOutput, _>(
        "proposal.execute",
        1,
        "Execute a previously approved proposal.",
        OperationEffect::PreparedApprovalExecution,
        OperationInputSchemaMode::Strict,
        |context: &RequestContext, input: PreparedExecutionInput| {
            let approval_id =
                context.approved_effect().map(|approved| approved.approval_id().to_owned());
            async move {
                let _proposal_id = input.proposal_id;
                let approval_id = approval_id.ok_or_else(|| {
                    OperationHandlerError::new("approval_required", "approval is required")
                })?;
                Ok(PreparedExecutionOutput { approval_id })
            }
        },
    )
    .expect("prepared execution registration should be valid")
}

fn counting_prepared_execution_registration() -> (OperationRegistration, Arc<AtomicUsize>) {
    let executions = Arc::new(AtomicUsize::new(0));
    let handler_executions = Arc::clone(&executions);
    let registration =
        OperationRegistration::new::<PreparedExecutionInput, PreparedExecutionOutput, _>(
            "proposal.execute",
            1,
            "Execute a previously approved proposal.",
            OperationEffect::PreparedApprovalExecution,
            OperationInputSchemaMode::Strict,
            move |_context: &RequestContext, input: PreparedExecutionInput| {
                let executions = Arc::clone(&handler_executions);
                async move {
                    executions.fetch_add(1, Ordering::SeqCst);
                    Ok(PreparedExecutionOutput { approval_id: input.proposal_id })
                }
            },
        )
        .expect("prepared execution registration should be valid");
    (registration, executions)
}

fn request_context(approved_effect: Option<ApprovedEffect>) -> RequestContext {
    request_context_with_version(1, approved_effect)
}

fn request_context_with_version(
    tool_version: u32,
    approved_effect: Option<ApprovedEffect>,
) -> RequestContext {
    RequestContext::new("project-7", "session-3", "request-11", tool_version, approved_effect)
}
