use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationRegistration,
    RequestContext,
};
use oh_my_dream_tauri::assistant_runtime::{
    AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome, AssistantSidecarCommand,
    AssistantWaitingApproval, TrustedInvocationContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalReadInput {
    pub query: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalReadOutput {
    pub project_id: String,
    pub request_id: String,
    pub result: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreparedInput {
    proposal_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreparedOutput {
    result: String,
}

pub async fn waiting_approval(
    runtime: &AssistantRuntime,
    session_path: &Path,
) -> AssistantWaitingApproval {
    let outcome = runtime
        .invoke(
            AssistantInvocation::new(
                "invoke-pause",
                "approval-session",
                session_path,
                Some("Execute the proposal.".to_owned()),
            ),
            trusted_context(),
        )
        .await
        .expect("approval invocation should pause");
    match outcome {
        AssistantRuntimeOutcome::WaitingApproval(waiting) => waiting,
        AssistantRuntimeOutcome::Completed(_) => panic!("expected waiting approval outcome"),
    }
}

pub fn approval_runtime(executions: Arc<AtomicUsize>) -> AssistantRuntime {
    AssistantRuntime::new(
        python_fixture_command("approval"),
        vec![approval_registration(executions)],
    )
    .expect("approval runtime should be valid")
}

pub fn approval_registration(executions: Arc<AtomicUsize>) -> OperationRegistration {
    prepared_registration("proposal_execute", 3, executions)
}

pub fn prepared_registration(
    operation_id: &str,
    version: u32,
    executions: Arc<AtomicUsize>,
) -> OperationRegistration {
    OperationRegistration::new::<PreparedInput, PreparedOutput, _>(
        operation_id,
        version,
        "Execute a prepared proposal.",
        OperationEffect::PreparedApprovalExecution,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: PreparedInput| {
            let executions = Arc::clone(&executions);
            let approved = context.approved_effect().map(|effect| effect.approval_id().to_owned());
            async move {
                if approved.as_deref() != Some("call-1") {
                    return Err(OperationHandlerError::new(
                        "APPROVAL_MISSING",
                        "trusted approval was not supplied",
                    ));
                }
                executions.fetch_add(1, Ordering::SeqCst);
                Ok(PreparedOutput { result: input.proposal_id })
            }
        },
    )
    .expect("approval registration should be valid")
}

pub fn trusted_context() -> TrustedInvocationContext {
    TrustedInvocationContext::new("project-7", "request-9")
}

pub async fn invoke_without_operations(
    runtime: &AssistantRuntime,
    invocation_id: &str,
) -> Result<AssistantRuntimeOutcome, oh_my_dream_tauri::assistant_runtime::AssistantRuntimeError> {
    let directory = TempDir::new().expect("temporary directory should be created");
    runtime
        .invoke(
            AssistantInvocation::new(
                invocation_id,
                "session",
                directory.path().join("session.sqlite3"),
                Some("input".to_owned()),
            ),
            trusted_context(),
        )
        .await
}

pub fn python_fixture_command(mode: &str) -> AssistantSidecarCommand {
    AssistantSidecarCommand::new("python3")
        .args(["-m", "assistant.tests.agent_transport_fixture", mode])
        .current_dir(repository_root())
}

pub fn malformed_command() -> AssistantSidecarCommand {
    AssistantSidecarCommand::new("python3").args([
        "-c",
        "import sys; sys.stdin.readline(); sys.stdout.write('not-json\\n'); sys.stdout.flush()",
    ])
}

pub fn hostile_command(script: &str) -> AssistantSidecarCommand {
    AssistantSidecarCommand::new("python3").args(["-c", script])
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri should have a repository parent")
        .to_owned()
}
