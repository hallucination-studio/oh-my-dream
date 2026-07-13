use std::time::Duration;

use oh_my_dream_tauri::assistant_sidecar::{AssistantStdioSidecar, AssistantStdioSidecarError};
use oh_my_dream_tauri::assistant_transport::{
    AssistantFrame, AssistantFrameKind, AssistantTransportError,
};
use serde_json::json;
use tokio::process::Command;
use tokio::time::timeout;

#[tokio::test]
async fn assistant_transport_assistant_sidecar_round_trips_and_shuts_down() {
    let mut sidecar = AssistantStdioSidecar::launch(fixture_command("echo"))
        .await
        .expect("stdio sidecar should launch");
    let frame = AssistantFrame::new(0, AssistantFrameKind::Invoke, json!({"prompt": "draw"}))
        .expect("test frame should be valid");

    sidecar
        .writer_mut()
        .expect("sidecar stdin should be open")
        .write_frame(&frame)
        .await
        .expect("frame should write");
    let echoed = sidecar.reader_mut().read_frame().await.expect("frame should echo");
    assert_eq!(echoed, frame);

    let status = sidecar.shutdown().await.expect("sidecar should be reaped");
    assert!(status.success());
}

#[tokio::test]
async fn assistant_transport_assistant_sidecar_kills_and_waits_for_child() {
    let mut sidecar = AssistantStdioSidecar::launch(fixture_command("wait"))
        .await
        .expect("stdio sidecar should launch");

    sidecar.kill().await.expect("sidecar should be killed");
    let status = sidecar.wait().await.expect("sidecar should be reaped");

    assert!(!status.success());
}

#[tokio::test]
async fn assistant_transport_assistant_sidecar_shutdown_drains_large_output() {
    let sidecar = AssistantStdioSidecar::launch(fixture_command("large-output"))
        .await
        .expect("stdio sidecar should launch");

    let status = timeout(Duration::from_secs(5), sidecar.shutdown())
        .await
        .expect("shutdown must not deadlock on a full stdout pipe")
        .expect("large protocol output should remain valid");

    assert!(status.success());
}

#[tokio::test]
async fn assistant_transport_assistant_sidecar_shutdown_returns_protocol_errors() {
    let sidecar = AssistantStdioSidecar::launch(fixture_command("invalid-output"))
        .await
        .expect("stdio sidecar should launch");

    let error = timeout(Duration::from_secs(5), sidecar.shutdown())
        .await
        .expect("shutdown must complete after invalid output")
        .expect_err("invalid terminal output must fail shutdown");

    assert!(matches!(
        error,
        AssistantStdioSidecarError::Transport {
            source: AssistantTransportError::MalformedJson { .. }
        }
    ));
}

#[tokio::test]
async fn assistant_transport_assistant_sidecar_drains_after_protocol_error() {
    let sidecar = AssistantStdioSidecar::launch(fixture_command("invalid-then-large-output"))
        .await
        .expect("stdio sidecar should launch");

    let error = timeout(Duration::from_secs(5), sidecar.shutdown())
        .await
        .expect("shutdown must drain output after a protocol error")
        .expect_err("the first protocol error must be preserved");

    assert!(matches!(
        error,
        AssistantStdioSidecarError::Transport {
            source: AssistantTransportError::MalformedJson { .. }
        }
    ));
}

fn fixture_command(mode: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_assistant_transport_fixture"));
    command.arg(mode);
    command
}
