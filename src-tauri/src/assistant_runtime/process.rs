use async_trait::async_trait;

use crate::assistant_sidecar::AssistantStdioSidecar;
use crate::assistant_transport::AssistantFrame;

use super::{AssistantRuntimeError, AssistantSidecarCommand};

/// One launched assistant process as consumed by the runtime orchestrator.
#[async_trait]
pub trait AssistantProcess: Send {
    /// Reads the next validated sidecar frame.
    async fn read_frame(&mut self) -> Result<AssistantFrame, AssistantRuntimeError>;

    /// Writes one validated sidecar frame.
    async fn write_frame(&mut self, frame: &AssistantFrame) -> Result<(), AssistantRuntimeError>;

    /// Requires protocol EOF and a successful child exit.
    async fn shutdown(&mut self) -> Result<(), AssistantRuntimeError>;

    /// Terminates and reaps the process after a runtime failure.
    async fn abort(&mut self);
}

/// Consumer-owned factory for a fresh assistant process per invocation.
#[async_trait]
pub trait AssistantProcessLauncher: Send + Sync {
    /// Launches one isolated assistant process.
    ///
    /// Implementations must clean up any partially launched process when this
    /// future is cancelled by the invocation deadline.
    async fn launch(&self) -> Result<Box<dyn AssistantProcess>, AssistantRuntimeError>;
}

struct StdioAssistantProcess {
    sidecar: AssistantStdioSidecar,
}

#[async_trait]
impl AssistantProcessLauncher for AssistantSidecarCommand {
    async fn launch(&self) -> Result<Box<dyn AssistantProcess>, AssistantRuntimeError> {
        let sidecar = AssistantStdioSidecar::launch(self.command()).await?;
        Ok(Box::new(StdioAssistantProcess { sidecar }))
    }
}

#[async_trait]
impl AssistantProcess for StdioAssistantProcess {
    async fn read_frame(&mut self) -> Result<AssistantFrame, AssistantRuntimeError> {
        Ok(self.sidecar.reader_mut().read_frame().await?)
    }

    async fn write_frame(&mut self, frame: &AssistantFrame) -> Result<(), AssistantRuntimeError> {
        let writer = self
            .sidecar
            .writer_mut()
            .ok_or(crate::assistant_sidecar::AssistantStdioSidecarError::MissingStdin)?;
        writer.write_frame(frame).await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), AssistantRuntimeError> {
        let status = self.sidecar.shutdown_strict().await?;
        if status.success() {
            return Ok(());
        }
        Err(AssistantRuntimeError::ProcessExit { status })
    }

    async fn abort(&mut self) {
        if let Err(error) = self.sidecar.kill().await {
            tracing::warn!(error = %error, "failed to kill assistant sidecar after runtime error");
        }
        if let Err(error) = self.sidecar.wait().await {
            tracing::warn!(error = %error, "failed to reap assistant sidecar after runtime error");
        }
    }
}
