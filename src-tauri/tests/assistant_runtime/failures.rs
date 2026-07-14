use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use oh_my_dream_tauri::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationRegistration, RequestContext,
};
use oh_my_dream_tauri::assistant_runtime::{
    AssistantInvocation, AssistantRuntime, AssistantRuntimeError, AssistantRuntimeLimits,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde::ser::Error as _;
use tempfile::TempDir;

use super::common::{
    LocalReadInput, PreparedOutput, approval_registration, hostile_command,
    invoke_without_operations, malformed_command, python_fixture_command, trusted_context,
};

#[tokio::test]
async fn assistant_runtime_unknown_operation_fails_closed() {
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); print(json.dumps({'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{'invocation_id':'unknown-op','operation_id':'not_registered','call_id':'call-1','arguments_json':'{}'}}),flush=True); sys.stdin.read()",
        ),
        Vec::new(),
    )
    .expect("runtime should be valid");
    let directory = TempDir::new().expect("temporary directory should be created");
    let error = runtime
        .invoke(
            AssistantInvocation::new(
                "unknown-op",
                "session",
                directory.path().join("session.sqlite3"),
                Some("call unknown".to_owned()),
            ),
            trusted_context(),
        )
        .await
        .expect_err("unknown operation must fail closed");

    assert!(matches!(
        error,
        AssistantRuntimeError::UnknownOperation { operation_id }
            if operation_id == "not_registered"
    ));
}

struct FailingOutput;

impl Serialize for FailingOutput {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(S::Error::custom("deliberate output failure"))
    }
}

impl JsonSchema for FailingOutput {
    fn schema_name() -> String {
        "FailingOutput".to_owned()
    }

    fn json_schema(_generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::Schema::Object(schemars::schema_for!(PreparedOutput).schema)
    }
}

#[tokio::test]
async fn assistant_runtime_malformed_operation_output_is_an_error() {
    let registration = OperationRegistration::new::<LocalReadInput, FailingOutput, _>(
        "workspace_get_snapshot",
        3,
        "Fail output serialization.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        |_context: &RequestContext, _input: LocalReadInput| async { Ok(FailingOutput) },
    )
    .expect("failing output registration should be valid");
    let runtime = AssistantRuntime::new(python_fixture_command("tool"), vec![registration])
        .expect("runtime should be valid");
    let directory = TempDir::new().expect("temporary directory should be created");
    let error = runtime
        .invoke(
            AssistantInvocation::new(
                "bad-output",
                "session",
                directory.path().join("session.sqlite3"),
                Some("call tool".to_owned()),
            ),
            trusted_context(),
        )
        .await
        .expect_err("invalid registered output must fail closed");

    assert!(matches!(error, AssistantRuntimeError::Operation(_)));
}

#[test]
fn assistant_runtime_has_no_transcript_or_state_json_model() {
    let source = include_str!("../../src/assistant_runtime/types.rs");
    assert!(!source.contains("transcript"));
    assert!(!source.contains("state_json"));
}

#[tokio::test]
async fn assistant_runtime_malformed_sidecar_output_is_an_error() {
    let runtime = AssistantRuntime::new(malformed_command(), Vec::new())
        .expect("empty runtime should be valid");
    let directory = TempDir::new().expect("temporary directory should be created");
    let error = runtime
        .invoke(
            AssistantInvocation::new(
                "malformed",
                "session",
                directory.path().join("session.sqlite3"),
                Some("input".to_owned()),
            ),
            trusted_context(),
        )
        .await
        .expect_err("malformed protocol output must fail");

    assert!(matches!(error, AssistantRuntimeError::Transport(_)));
}

#[tokio::test]
async fn assistant_runtime_rejects_an_incoming_frame_flood() {
    let limits =
        AssistantRuntimeLimits::new(Duration::from_secs(1), Duration::from_secs(1), 2, 1024)
            .expect("test limits should be valid");
    let runtime = AssistantRuntime::with_limits(
        hostile_command(
            "import sys,json; invoke=json.loads(sys.stdin.readline()); invocation=invoke['payload']['invocation_id']; [print(json.dumps({'protocol_version':1,'sequence':sequence,'kind':'responses_event','payload':{'invocation_id':invocation,'event':{'type':'response.output_text.delta','delta':'x'}}}),flush=True) for sequence in range(3)]; sys.stdin.read()",
        ),
        Vec::new(),
        limits,
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "frame-flood")
        .await
        .expect_err("frame flood must exceed the invocation budget");

    assert!(matches!(
        error,
        AssistantRuntimeError::ResourceLimit { resource: "incoming frames", maximum: 2 }
    ));
}

#[tokio::test]
async fn assistant_runtime_rejects_token_bytes_over_budget() {
    let limits =
        AssistantRuntimeLimits::new(Duration::from_secs(1), Duration::from_secs(1), 8, 100)
            .expect("test limits should be valid");
    let runtime = AssistantRuntime::with_limits(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']['invocation_id']; print(json.dumps({'protocol_version':1,'sequence':0,'kind':'responses_event','payload':{'invocation_id':invocation,'event':{'type':'response.output_text.delta','delta':'x'*200}}}),flush=True); sys.stdin.read()",
        ),
        Vec::new(),
        limits,
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "token-budget")
        .await
        .expect_err("oversized token stream must exceed the byte budget");

    assert!(matches!(
        error,
        AssistantRuntimeError::ResourceLimit { resource: "invocation bytes", maximum: 100 }
    ));
}

#[tokio::test]
async fn assistant_runtime_rejects_invalid_token_correlation() {
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; sys.stdin.readline(); print(json.dumps({'protocol_version':1,'sequence':0,'kind':'responses_event','payload':{'invocation_id':'wrong','event':{'type':'response.output_text.delta','delta':'x'}}}),flush=True); sys.stdin.read()",
        ),
        Vec::new(),
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "token-correlation")
        .await
        .expect_err("token correlation mismatch must fail closed");

    assert!(matches!(error, AssistantRuntimeError::InvocationMismatch { .. }));
}

#[tokio::test]
async fn assistant_runtime_rejects_frames_after_completion() {
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']; frames=[('snapshot',{'invocation_id':invocation['invocation_id'],'session_id':invocation['session_id'],'status':'completed','state':None}),('completed',{'invocation_id':invocation['invocation_id'],'final_output':'done'}),('responses_event',{'invocation_id':invocation['invocation_id'],'event':{'type':'response.output_text.delta','delta':'late'}})]; [print(json.dumps({'protocol_version':1,'sequence':sequence,'kind':kind,'payload':payload}),flush=True) for sequence,(kind,payload) in enumerate(frames)]",
        ),
        Vec::new(),
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "trailing-frame")
        .await
        .expect_err("trailing frames must invalidate completion");

    assert!(matches!(error, AssistantRuntimeError::Sidecar(_)));
}

#[tokio::test]
async fn assistant_runtime_rejects_nonzero_exit_after_completion() {
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']; frames=[('snapshot',{'invocation_id':invocation['invocation_id'],'session_id':invocation['session_id'],'status':'completed','state':None}),('completed',{'invocation_id':invocation['invocation_id'],'final_output':'done'})]; [print(json.dumps({'protocol_version':1,'sequence':sequence,'kind':kind,'payload':payload}),flush=True) for sequence,(kind,payload) in enumerate(frames)]; raise SystemExit(7)",
        ),
        Vec::new(),
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "nonzero-exit")
        .await
        .expect_err("nonzero sidecar exit must invalidate completion");

    assert!(matches!(error, AssistantRuntimeError::ProcessExit { .. }));
}

#[tokio::test]
async fn assistant_runtime_rejects_duplicate_snapshots() {
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']; frame={'protocol_version':1,'sequence':0,'kind':'snapshot','payload':{'invocation_id':invocation['invocation_id'],'session_id':invocation['session_id'],'status':'completed','state':None}}; print(json.dumps(frame),flush=True); frame['sequence']=1; print(json.dumps(frame),flush=True); sys.stdin.read()",
        ),
        Vec::new(),
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "duplicate-snapshot")
        .await
        .expect_err("duplicate snapshots must fail closed");

    assert!(matches!(error, AssistantRuntimeError::InvalidStateTransition { .. }));
}

#[tokio::test]
async fn assistant_runtime_rejects_conflicting_approval_terminal_state() {
    let registration = approval_registration(Arc::new(AtomicUsize::new(0)));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']; approval={'protocol_version':1,'sequence':0,'kind':'approval_request','payload':{'invocation_id':invocation['invocation_id'],'operation_id':'proposal_execute','call_id':'call-1','arguments_json':'{\\\"proposal_id\\\":\\\"proposal-42\\\"}','state':{}}}; print(json.dumps(approval),flush=True); snapshot={'protocol_version':1,'sequence':1,'kind':'snapshot','payload':{'invocation_id':invocation['invocation_id'],'session_id':invocation['session_id'],'status':'completed','state':{}}}; print(json.dumps(snapshot),flush=True); sys.stdin.read()",
        ),
        vec![registration],
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "approval-conflict")
        .await
        .expect_err("approval cannot resolve through a completed snapshot");

    assert!(matches!(error, AssistantRuntimeError::InvalidStateTransition { .. }));
}

#[tokio::test]
async fn assistant_runtime_rejects_duplicate_approval_requests() {
    let registration = approval_registration(Arc::new(AtomicUsize::new(0)));
    let runtime = AssistantRuntime::new(
        hostile_command(
            "import sys,json; invocation=json.loads(sys.stdin.readline())['payload']; frame={'protocol_version':1,'sequence':0,'kind':'approval_request','payload':{'invocation_id':invocation['invocation_id'],'operation_id':'proposal_execute','call_id':'call-1','arguments_json':'{\\\"proposal_id\\\":\\\"proposal-42\\\"}','state':{}}}; print(json.dumps(frame),flush=True); frame['sequence']=1; print(json.dumps(frame),flush=True); sys.stdin.read()",
        ),
        vec![registration],
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "duplicate-approval")
        .await
        .expect_err("duplicate approval requests must fail closed");

    assert!(matches!(error, AssistantRuntimeError::InvalidStateTransition { .. }));
}
