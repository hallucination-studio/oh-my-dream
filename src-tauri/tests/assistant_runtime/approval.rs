use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use oh_my_dream_tauri::assistant_runtime::{
    AssistantInvocation, AssistantRuntime, AssistantRuntimeError, AssistantRuntimeOutcome,
    TrustedInvocationContext,
};
use tempfile::TempDir;

use super::common::{
    approval_registration, approval_runtime, hostile_command, prepared_registration,
    trusted_context, waiting_approval,
};

#[tokio::test]
async fn assistant_runtime_resumes_approval_in_fresh_process_and_executes_once() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("approval.sqlite3");
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = approval_runtime(Arc::clone(&executions));
    let waiting = waiting_approval(&runtime, &session_path).await;

    assert_eq!(waiting.pending().call_id(), "call-1");
    assert_eq!(waiting.pending().operation_id(), "proposal_execute");
    assert_eq!(waiting.pending().operation_version(), 3);
    assert_eq!(waiting.pending().arguments_json(), "{\n  \"proposal_id\": \"proposal-42\"\n}");
    assert_eq!(waiting.state()["envelope_version"], 1);
    assert!(waiting.state().get("state_json").is_some());
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    let resumed = runtime
        .resume(
            AssistantInvocation::new("invoke-resume", "approval-session", &session_path, None),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect("approved invocation should complete");
    let AssistantRuntimeOutcome::Completed(completed) = resumed else {
        panic!("expected completed approval outcome");
    };
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(completed.operation_calls().len(), 1);
    assert_eq!(completed.operation_calls()[0].operation_version(), 3);
    assert!(session_path.exists());
}

#[tokio::test]
async fn assistant_runtime_rejected_approval_does_not_dispatch() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("rejected.sqlite3");
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = approval_runtime(Arc::clone(&executions));
    let waiting = waiting_approval(&runtime, &session_path).await;

    let outcome = runtime
        .resume(
            AssistantInvocation::new("invoke-reject", "approval-session", &session_path, None),
            trusted_context(),
            waiting,
            false,
        )
        .await
        .expect("rejected invocation should complete");

    assert!(matches!(outcome, AssistantRuntimeOutcome::Completed(_)));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_runtime_approval_call_mismatch_fails_closed() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("mismatch.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); sys.stdin.readline(); print(json.dumps({'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{'invocation_id':'invoke-mismatch','operation_id':'proposal_execute','call_id':'different-call','arguments_json':'{\\\"proposal_id\\\":\\\"proposal-42\\\"}'}}),flush=True); sys.stdin.read()",
        ),
        vec![approval_registration(Arc::clone(&executions))],
    )
    .expect("hostile runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new("invoke-mismatch", "approval-session", &session_path, None),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect_err("mismatched approval identity must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalMismatch));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_runtime_approval_operation_mismatch_fails_closed() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("operation-mismatch.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); sys.stdin.readline(); print(json.dumps({'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{'invocation_id':'invoke-operation-mismatch','operation_id':'other_execute','call_id':'call-1','arguments_json':'{\\\"proposal_id\\\":\\\"proposal-42\\\"}'}}),flush=True); sys.stdin.read()",
        ),
        vec![
            approval_registration(Arc::new(AtomicUsize::new(0))),
            prepared_registration("other_execute", 3, Arc::clone(&executions)),
        ],
    )
    .expect("hostile runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new(
                "invoke-operation-mismatch",
                "approval-session",
                &session_path,
                None,
            ),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect_err("mismatched approval operation must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalMismatch));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_runtime_approval_arguments_mismatch_fails_closed() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("arguments-mismatch.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); sys.stdin.readline(); print(json.dumps({'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{'invocation_id':'invoke-arguments-mismatch','operation_id':'proposal_execute','call_id':'call-1','arguments_json':'{\\\"proposal_id\\\":\\\"proposal-99\\\"}'}}),flush=True); sys.stdin.read()",
        ),
        vec![approval_registration(Arc::clone(&executions))],
    )
    .expect("hostile runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new(
                "invoke-arguments-mismatch",
                "approval-session",
                &session_path,
                None,
            ),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect_err("changed approved arguments must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalMismatch));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_runtime_approval_scope_mismatch_fails_before_launch() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("scope-mismatch.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let runtime = AssistantRuntime::new(
        hostile_command("raise SystemExit('must not launch')"),
        vec![approval_registration(Arc::new(AtomicUsize::new(0)))],
    )
    .expect("scope mismatch runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new(
                "invoke-scope-mismatch",
                "approval-session",
                &session_path,
                None,
            ),
            TrustedInvocationContext::new("different-project", "request-9"),
            waiting,
            true,
        )
        .await
        .expect_err("cross-project approval replay must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalScopeMismatch));
}

#[tokio::test]
async fn assistant_runtime_approval_version_mismatch_fails_before_launch() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("version-mismatch.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let runtime = AssistantRuntime::new(
        hostile_command("raise SystemExit('must not launch')"),
        vec![prepared_registration("proposal_execute", 4, Arc::new(AtomicUsize::new(0)))],
    )
    .expect("version mismatch runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new(
                "invoke-version-mismatch",
                "approval-session",
                &session_path,
                None,
            ),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect_err("mismatched approval version must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalMismatch));
}

#[tokio::test]
async fn assistant_runtime_approved_effect_cannot_be_reused() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let session_path = directory.path().join("reuse.sqlite3");
    let waiting =
        waiting_approval(&approval_runtime(Arc::new(AtomicUsize::new(0))), &session_path).await;
    let executions = Arc::new(AtomicUsize::new(0));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); sys.stdin.readline(); arguments='{\\n  \\\"proposal_id\\\": \\\"proposal-42\\\"\\n}'; frame={'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{'invocation_id':'invoke-reuse','operation_id':'proposal_execute','call_id':'call-1','arguments_json':arguments}}; print(json.dumps(frame),flush=True); sys.stdin.readline(); frame['sequence']=1; print(json.dumps(frame),flush=True); sys.stdin.read()",
        ),
        vec![approval_registration(Arc::clone(&executions))],
    )
    .expect("hostile runtime should be valid");
    let error = runtime
        .resume(
            AssistantInvocation::new("invoke-reuse", "approval-session", &session_path, None),
            trusted_context(),
            waiting,
            true,
        )
        .await
        .expect_err("approved effect reuse must fail closed");

    assert!(matches!(error, AssistantRuntimeError::ApprovalReuse));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}
