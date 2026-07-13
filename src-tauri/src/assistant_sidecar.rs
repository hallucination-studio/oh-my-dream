use crate::assistant_transport::{
    AssistantFrameReader, AssistantFrameWriter, AssistantTransportError,
};
use crate::dto::AssistantSessionDto;
use crate::state::AppState;
use std::io::{self, BufRead};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
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

/// Creates an assistant session, spawning the sidecar when app state enables it.
pub fn create_assistant_session(
    state: &AppState,
) -> Result<(AssistantSessionDto, Option<Child>), String> {
    let token = generate_token()?;
    let (port, process) = if state.assistant_sidecar_enabled {
        spawn_assistant_sidecar(state, &token)?
    } else {
        (reserve_loopback_port()?, None)
    };
    Ok((AssistantSessionDto { port, token }, process))
}

fn reserve_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|source| assistant_error("reserve assistant port", source))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|source| assistant_error("read assistant port", source))
}

fn spawn_assistant_sidecar(state: &AppState, token: &str) -> Result<(u16, Option<Child>), String> {
    let mut child = Command::new("python3")
        .args(["-m", "assistant"])
        .env("OH_MY_DREAM_ASSISTANT_TOKEN", token)
        .env("OH_MY_DREAM_CONFIG_ROOT", &state.config_root)
        .env("OH_MY_DREAM_ALLOWED_ORIGINS", "tauri://localhost,http://localhost:5273")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| assistant_error("spawn assistant sidecar", source))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| assistant_error("read assistant sidecar stdout", "stdout was not piped"))?;
    let mut line = String::new();
    std::io::BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|source| assistant_error("read assistant sidecar port", source))?;
    let port = line
        .strip_prefix("PORT=")
        .and_then(|value| value.trim().parse::<u16>().ok())
        .ok_or_else(|| assistant_error("parse assistant sidecar port", line.trim()))?;
    Ok((port, Some(child)))
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|source| assistant_error("generate assistant token", source))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "assistant sidecar failed");
    format!("{operation}: {error}")
}
