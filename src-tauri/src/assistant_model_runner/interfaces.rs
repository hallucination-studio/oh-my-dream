use assistant::{domain::AssistantSessionId, interfaces::AssistantApplicationError};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use serde_json::Value;

/// Executes one exact model-requested tool through Rust-owned handlers.
#[async_trait]
pub trait AssistantProtocolToolExecutorInterface: Send + Sync {
    async fn execute_assistant_protocol_tool(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        tool_id: &str,
        arguments: Value,
    ) -> Result<Value, AssistantApplicationError>;
}

/// One isolated raw Assistant protocol process.
#[async_trait]
pub trait AssistantProtocolProcessInterface: Send {
    async fn read_assistant_protocol_line(&mut self) -> Result<Vec<u8>, AssistantApplicationError>;
    async fn write_assistant_protocol_line(
        &mut self,
        encoded: &[u8],
    ) -> Result<(), AssistantApplicationError>;
    async fn shutdown_assistant_protocol_process(
        &mut self,
    ) -> Result<(), AssistantApplicationError>;
    async fn abort_assistant_protocol_process(&mut self);
}

/// Launches one isolated Assistant protocol process per invocation.
#[async_trait]
pub trait AssistantProtocolProcessLauncherInterface: Send + Sync {
    async fn launch_assistant_protocol_process(
        &self,
    ) -> Result<Box<dyn AssistantProtocolProcessInterface>, AssistantApplicationError>;
}
