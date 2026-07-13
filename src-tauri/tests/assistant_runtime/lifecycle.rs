use std::collections::VecDeque;
use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use oh_my_dream_tauri::assistant_runtime::{
    AssistantProcess, AssistantProcessLauncher, AssistantRuntime, AssistantRuntimeError,
    AssistantRuntimeLimits,
};
use oh_my_dream_tauri::assistant_transport::{AssistantFrame, AssistantFrameKind};

use super::common::{hostile_command, invoke_without_operations};

#[tokio::test]
async fn assistant_runtime_times_out_a_stalled_sidecar() {
    let limits =
        AssistantRuntimeLimits::new(Duration::from_millis(50), Duration::from_secs(1), 8, 1024)
            .expect("test limits should be valid");
    let runtime = AssistantRuntime::with_limits(
        hostile_command("import sys,time; sys.stdin.readline(); time.sleep(60)"),
        Vec::new(),
        limits,
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "timeout")
        .await
        .expect_err("stalled sidecar must time out");

    assert!(matches!(error, AssistantRuntimeError::InvocationTimeout));
}

struct HangingLauncher;

#[async_trait]
impl AssistantProcessLauncher for HangingLauncher {
    async fn launch(&self) -> Result<Box<dyn AssistantProcess>, AssistantRuntimeError> {
        pending().await
    }
}

#[tokio::test]
async fn assistant_runtime_times_out_a_stalled_launcher() {
    let limits =
        AssistantRuntimeLimits::new(Duration::from_millis(50), Duration::from_secs(1), 8, 1024)
            .expect("test limits should be valid");
    let runtime = AssistantRuntime::with_limits(HangingLauncher, Vec::new(), limits)
        .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "launch-timeout")
        .await
        .expect_err("stalled launcher must time out");

    assert!(matches!(error, AssistantRuntimeError::InvocationTimeout));
}

#[tokio::test]
async fn assistant_runtime_aborts_a_sidecar_when_shutdown_times_out() {
    let limits =
        AssistantRuntimeLimits::new(Duration::from_secs(1), Duration::from_millis(50), 8, 1024)
            .expect("test limits should be valid");
    let runtime = AssistantRuntime::with_limits(
        hostile_command(
            "import sys,json,time; invocation=json.loads(sys.stdin.readline())['payload']; frames=[('snapshot',{'invocation_id':invocation['invocation_id'],'session_id':invocation['session_id'],'status':'completed','state':None}),('completed',{'invocation_id':invocation['invocation_id'],'final_output':'done'})]; [print(json.dumps({'protocol_version':1,'sequence':sequence,'kind':kind,'payload':payload}),flush=True) for sequence,(kind,payload) in enumerate(frames)]; time.sleep(60)",
        ),
        Vec::new(),
        limits,
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "shutdown-timeout")
        .await
        .expect_err("stalled shutdown must time out and abort");

    assert!(matches!(error, AssistantRuntimeError::ShutdownTimeout));
}

struct FailingShutdownLauncher {
    aborted: Arc<AtomicBool>,
}

struct FailingShutdownProcess {
    frames: VecDeque<AssistantFrame>,
    aborted: Arc<AtomicBool>,
}

#[async_trait]
impl AssistantProcessLauncher for FailingShutdownLauncher {
    async fn launch(&self) -> Result<Box<dyn AssistantProcess>, AssistantRuntimeError> {
        let frames = VecDeque::from([
            test_frame(
                0,
                AssistantFrameKind::Snapshot,
                serde_json::json!({
                    "invocation_id": "shutdown-error",
                    "session_id": "session",
                    "status": "completed",
                    "state": null
                }),
            )?,
            test_frame(
                1,
                AssistantFrameKind::Completed,
                serde_json::json!({
                    "invocation_id": "shutdown-error",
                    "final_output": "done"
                }),
            )?,
        ]);
        Ok(Box::new(FailingShutdownProcess { frames, aborted: Arc::clone(&self.aborted) }))
    }
}

#[async_trait]
impl AssistantProcess for FailingShutdownProcess {
    async fn read_frame(&mut self) -> Result<AssistantFrame, AssistantRuntimeError> {
        self.frames.pop_front().ok_or(AssistantRuntimeError::InvalidStateTransition {
            event: "test process ran out of frames",
        })
    }

    async fn write_frame(&mut self, _frame: &AssistantFrame) -> Result<(), AssistantRuntimeError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), AssistantRuntimeError> {
        Err(AssistantRuntimeError::InvalidStateTransition { event: "test shutdown failure" })
    }

    async fn abort(&mut self) {
        self.aborted.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn assistant_runtime_aborts_after_shutdown_error() {
    let aborted = Arc::new(AtomicBool::new(false));
    let runtime = AssistantRuntime::new(
        FailingShutdownLauncher { aborted: Arc::clone(&aborted) },
        Vec::new(),
    )
    .expect("runtime should be valid");
    let error = invoke_without_operations(&runtime, "shutdown-error")
        .await
        .expect_err("shutdown failure must remain an error");

    assert!(matches!(
        error,
        AssistantRuntimeError::InvalidStateTransition { event: "test shutdown failure" }
    ));
    assert!(aborted.load(Ordering::SeqCst));
}

fn test_frame(
    sequence: u64,
    kind: AssistantFrameKind,
    payload: serde_json::Value,
) -> Result<AssistantFrame, AssistantRuntimeError> {
    Ok(AssistantFrame::new(sequence, kind, payload)?)
}
