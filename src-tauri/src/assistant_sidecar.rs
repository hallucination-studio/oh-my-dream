use crate::assistant_runtime::AssistantSidecarCommand;
use crate::assistant_transport::{
    AssistantFrameReader, AssistantFrameWriter, AssistantTransportError,
};
use std::io;
use std::path::Path;
use std::process::Stdio;
use thiserror::Error;
use tokio::io::{BufReader as AsyncBufReader, BufWriter as AsyncBufWriter};
use tokio::process::{
    Child as AsyncChild, ChildStdin as AsyncChildStdin, ChildStdout as AsyncChildStdout,
    Command as AsyncCommand,
};
use tracing::error;

/// Spawn and lifecycle failures for the inherited-stdio assistant sidecar.
#[derive(Debug, Error)]
pub enum AssistantStdioSidecarError {
    #[error("failed to spawn assistant stdio sidecar")]
    Spawn {
        #[source]
        source: io::Error,
    },
    #[error("assistant stdio sidecar did not expose piped stdin")]
    MissingStdin,
    #[error("assistant stdio sidecar did not expose piped stdout")]
    MissingStdout,
    #[error("failed to kill assistant stdio sidecar")]
    Kill {
        #[source]
        source: io::Error,
    },
    #[error("failed to wait for assistant stdio sidecar")]
    Wait {
        #[source]
        source: io::Error,
    },
    #[error("assistant stdio sidecar emitted invalid protocol output")]
    Transport {
        #[source]
        source: AssistantTransportError,
    },
}

/// Owns one assistant child process and its framed stdin/stdout endpoints.
pub struct AssistantStdioSidecar {
    child: AsyncChild,
    reader: AssistantFrameReader<AsyncBufReader<AsyncChildStdout>>,
    writer: Option<AssistantFrameWriter<AsyncBufWriter<AsyncChildStdin>>>,
}

impl AssistantStdioSidecar {
    /// Spawns the supplied command with piped stdin/stdout and inherited stderr.
    pub async fn launch(mut command: AsyncCommand) -> Result<Self, AssistantStdioSidecarError> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| AssistantStdioSidecarError::Spawn { source })?;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                terminate_stdio_child(&mut child).await;
                return Err(AssistantStdioSidecarError::MissingStdin);
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                terminate_stdio_child(&mut child).await;
                return Err(AssistantStdioSidecarError::MissingStdout);
            }
        };
        Ok(Self {
            child,
            reader: AssistantFrameReader::new(AsyncBufReader::new(stdout)),
            writer: Some(AssistantFrameWriter::new(AsyncBufWriter::new(stdin))),
        })
    }

    /// Returns the owned child process.
    pub fn child(&self) -> &AsyncChild {
        &self.child
    }

    /// Returns the framed stdout reader.
    pub fn reader_mut(&mut self) -> &mut AssistantFrameReader<AsyncBufReader<AsyncChildStdout>> {
        &mut self.reader
    }

    /// Returns the framed stdin writer while it remains open.
    pub fn writer_mut(
        &mut self,
    ) -> Option<&mut AssistantFrameWriter<AsyncBufWriter<AsyncChildStdin>>> {
        self.writer.as_mut()
    }

    /// Closes stdin and waits for the child to exit.
    pub async fn shutdown(
        mut self,
    ) -> Result<std::process::ExitStatus, AssistantStdioSidecarError> {
        drop(self.writer.take());
        self.wait().await
    }

    /// Closes stdin, requires immediate protocol EOF, and waits for child exit.
    pub async fn shutdown_strict(
        &mut self,
    ) -> Result<std::process::ExitStatus, AssistantStdioSidecarError> {
        drop(self.writer.take());
        let reader = &mut self.reader;
        let child = &mut self.child;
        let eof = async {
            reader
                .expect_eof()
                .await
                .map_err(|source| AssistantStdioSidecarError::Transport { source })
        };
        let wait = async {
            child.wait().await.map_err(|source| AssistantStdioSidecarError::Wait { source })
        };
        let (eof_result, wait_result) = tokio::join!(eof, wait);
        eof_result?;
        wait_result
    }

    /// Kills the child process and waits for termination.
    pub async fn kill(&mut self) -> Result<(), AssistantStdioSidecarError> {
        drop(self.writer.take());
        self.child.kill().await.map_err(|source| AssistantStdioSidecarError::Kill { source })
    }

    /// Waits for the child process to exit and be reaped.
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, AssistantStdioSidecarError> {
        drop(self.writer.take());
        let reader = &mut self.reader;
        let child = &mut self.child;
        let drain = async {
            reader
                .drain_to_eof()
                .await
                .map_err(|source| AssistantStdioSidecarError::Transport { source })
        };
        let wait = async {
            child.wait().await.map_err(|source| AssistantStdioSidecarError::Wait { source })
        };
        let (drain_result, wait_result) = tokio::join!(drain, wait);
        drain_result?;
        wait_result
    }
}

async fn terminate_stdio_child(child: &mut AsyncChild) {
    match child.try_wait() {
        Ok(Some(_status)) => return,
        Ok(None) => {}
        Err(source) => error!(error = %source, "failed to inspect assistant stdio sidecar"),
    }
    if let Err(source) = child.kill().await {
        error!(error = %source, "failed to terminate assistant stdio sidecar");
    }
    if let Err(source) = child.wait().await {
        error!(error = %source, "failed to reap assistant stdio sidecar");
    }
}

/// Resolves the shared stdio command for development or a packaged app.
pub fn configured_assistant_command() -> Result<AssistantSidecarCommand, String> {
    if cfg!(debug_assertions) {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| assistant_error("resolve assistant repository", "missing parent"))?;
        return Ok(AssistantSidecarCommand::development(
            std::env::var_os("OH_MY_DREAM_PYTHON").unwrap_or_else(|| "python3".into()),
            repository_root,
        ));
    }

    let current_executable = std::env::current_exe()
        .map_err(|source| assistant_error("resolve assistant executable", source))?;
    let target = option_env!("OH_MY_DREAM_TARGET_TRIPLE")
        .ok_or_else(|| assistant_error("resolve assistant target", "target triple is missing"))?;
    let executable = AssistantSidecarCommand::packaged_executable_path(current_executable, target);
    Ok(AssistantSidecarCommand::packaged(executable))
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "assistant sidecar failed");
    format!("{operation}: {error}")
}
