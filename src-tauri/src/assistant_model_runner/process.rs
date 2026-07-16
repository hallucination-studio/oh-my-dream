use std::{io, process::Stdio, sync::Arc};

use assistant::interfaces::AssistantApplicationError;
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStdin, ChildStdout},
};

use super::{AssistantProtocolProcessInterface, AssistantProtocolProcessLauncherInterface};
use crate::assistant_process_command::AssistantSidecarCommand;
use crate::credential_repository::{
    AssistantModelCredentialId, AssistantModelCredentialRepositoryInterface,
};

#[derive(Clone)]
pub struct AssistantSidecarCommandProcessLauncherImpl {
    command: AssistantSidecarCommand,
}

impl AssistantSidecarCommandProcessLauncherImpl {
    #[must_use]
    pub const fn new(command: AssistantSidecarCommand) -> Self {
        Self { command }
    }
}

struct StdioProtocolProcess {
    child: Child,
    stdin: Option<BufWriter<ChildStdin>>,
    stdout: BufReader<ChildStdout>,
}

#[async_trait]
impl AssistantProtocolProcessLauncherInterface for AssistantSidecarCommandProcessLauncherImpl {
    async fn launch_assistant_protocol_process(
        &self,
    ) -> Result<Box<dyn AssistantProtocolProcessInterface>, AssistantApplicationError> {
        let mut command = self.command.command();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(map_io)?;
        let Some(stdin) = child.stdin.take() else {
            terminate_child(&mut child).await;
            return Err(AssistantApplicationError::ModelUnavailable);
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_child(&mut child).await;
            return Err(AssistantApplicationError::ModelUnavailable);
        };
        Ok(Box::new(StdioProtocolProcess {
            child,
            stdin: Some(BufWriter::new(stdin)),
            stdout: BufReader::new(stdout),
        }))
    }
}

/// Loads the plaintext credential only for one bounded sidecar invocation.
#[derive(Clone)]
pub struct CredentialedAssistantSidecarProcessLauncherAdapterImpl {
    command: AssistantSidecarCommand,
    credentials: Arc<dyn AssistantModelCredentialRepositoryInterface>,
    credential_id: AssistantModelCredentialId,
    enabled: bool,
}

impl CredentialedAssistantSidecarProcessLauncherAdapterImpl {
    #[must_use]
    pub fn new(
        command: AssistantSidecarCommand,
        credentials: Arc<dyn AssistantModelCredentialRepositoryInterface>,
        credential_id: AssistantModelCredentialId,
        enabled: bool,
    ) -> Self {
        Self { command, credentials, credential_id, enabled }
    }
}

#[async_trait]
impl AssistantProtocolProcessLauncherInterface
    for CredentialedAssistantSidecarProcessLauncherAdapterImpl
{
    async fn launch_assistant_protocol_process(
        &self,
    ) -> Result<Box<dyn AssistantProtocolProcessInterface>, AssistantApplicationError> {
        if !self.enabled {
            return Err(AssistantApplicationError::ModelUnavailable);
        }
        let secret = self
            .credentials
            .load_assistant_model_credential(&self.credential_id)
            .await
            .map_err(|_| AssistantApplicationError::ModelUnavailable)?;
        let value = std::str::from_utf8(secret.as_bytes())
            .map_err(|_| AssistantApplicationError::ModelUnavailable)?;
        let launcher = AssistantSidecarCommandProcessLauncherImpl::new(
            self.command.clone().env("OMD_ASSISTANT_API_KEY", value),
        );
        launcher.launch_assistant_protocol_process().await
    }
}

#[async_trait]
impl AssistantProtocolProcessInterface for StdioProtocolProcess {
    async fn read_assistant_protocol_line(&mut self) -> Result<Vec<u8>, AssistantApplicationError> {
        let mut encoded = Vec::new();
        let mut bounded = (&mut self.stdout)
            .take((assistant::protocol_v1::MAX_ASSISTANT_PROTOCOL_FRAME_BYTES + 1) as u64);
        let read = bounded.read_until(b'\n', &mut encoded).await.map_err(map_io)?;
        if read == 0 {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        if encoded.len() > assistant::protocol_v1::MAX_ASSISTANT_PROTOCOL_FRAME_BYTES {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        Ok(encoded)
    }

    async fn write_assistant_protocol_line(
        &mut self,
        encoded: &[u8],
    ) -> Result<(), AssistantApplicationError> {
        let stdin = self.stdin.as_mut().ok_or(AssistantApplicationError::ModelUnavailable)?;
        stdin.write_all(encoded).await.map_err(map_io)?;
        stdin.flush().await.map_err(map_io)
    }

    async fn shutdown_assistant_protocol_process(
        &mut self,
    ) -> Result<(), AssistantApplicationError> {
        self.stdin.take();
        let mut trailing = [0_u8; 1];
        let (read, status) = tokio::join!(self.stdout.read(&mut trailing), self.child.wait());
        if read.map_err(map_io)? != 0 {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        if status.map_err(map_io)?.success() {
            Ok(())
        } else {
            Err(AssistantApplicationError::ModelUnavailable)
        }
    }

    async fn abort_assistant_protocol_process(&mut self) {
        self.stdin.take();
        if self.child.kill().await.is_err() {
            return;
        }
        let _ = self.child.wait().await;
    }
}

fn map_io(_error: io::Error) -> AssistantApplicationError {
    AssistantApplicationError::ModelUnavailable
}

async fn terminate_child(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}
