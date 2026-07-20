//! Isolated Python OpenAI provider-control process adapter.

use std::{process::Stdio, time::Duration};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::io::AsyncReadExt;

use crate::{
    assistant_process_command::AssistantSidecarCommand,
    assistant_provider_settings::{
        AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
        AssistantProviderProbeError, AssistantProviderProbeInterface,
    },
};

const PROVIDER_CONTROL_DEADLINE: Duration = Duration::from_secs(20);
const MAX_PROVIDER_CONTROL_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_PROVIDER_MODEL_COUNT: usize = 10_000;

/// Python sidecar implementation of bounded Assistant provider discovery and testing.
#[derive(Clone)]
pub struct PythonOpenAiAssistantProviderAdapterImpl {
    command: AssistantSidecarCommand,
    deadline: Duration,
}

impl PythonOpenAiAssistantProviderAdapterImpl {
    /// Uses the selected sidecar command and production provider-control deadline.
    #[must_use]
    pub const fn new(command: AssistantSidecarCommand) -> Self {
        Self { command, deadline: PROVIDER_CONTROL_DEADLINE }
    }

    #[cfg(test)]
    pub(crate) const fn with_deadline(
        command: AssistantSidecarCommand,
        deadline: Duration,
    ) -> Self {
        Self { command, deadline }
    }

    async fn execute(
        &self,
        operation: ProviderControlOperation<'_>,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
    ) -> Result<ProviderControlOutput, AssistantProviderProbeError> {
        let api_key = std::str::from_utf8(api_key.as_bytes())
            .map_err(|_| AssistantProviderProbeError::AuthenticationRejected)?;
        let mut command = self
            .command
            .clone()
            .env("OH_MY_DREAM_ASSISTANT_MODE", "provider_control")
            .env("OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION", operation.action())
            .env("OH_MY_DREAM_ASSISTANT_PROVIDER_BASE_URL", base_url.as_str())
            .env("OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY", api_key);
        if let Some(model_id) = operation.model_id() {
            command = command.env("OH_MY_DREAM_ASSISTANT_PROVIDER_MODEL_ID", model_id.as_str());
        }
        let mut child = command
            .command()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| AssistantProviderProbeError::ProviderUnreachable)?;
        let stdout = child.stdout.take().ok_or(AssistantProviderProbeError::ProviderUnreachable)?;
        let reader = tokio::spawn(async move {
            let mut output = Vec::new();
            stdout
                .take((MAX_PROVIDER_CONTROL_OUTPUT_BYTES + 1) as u64)
                .read_to_end(&mut output)
                .await
                .map(|_| output)
        });
        let execution = async {
            let output = reader
                .await
                .map_err(|_| malformed(operation))?
                .map_err(|_| malformed(operation))?;
            if output.len() > MAX_PROVIDER_CONTROL_OUTPUT_BYTES {
                terminate_child(&mut child).await;
                return Err(malformed(operation));
            }
            let status = child.wait().await.map_err(|_| malformed(operation))?;
            if !status.success() {
                return Err(AssistantProviderProbeError::ProviderUnreachable);
            }
            decode_output(operation, &output)
        };
        match tokio::time::timeout(self.deadline, execution).await {
            Ok(result) => result,
            Err(_) => {
                terminate_child(&mut child).await;
                Err(AssistantProviderProbeError::ProviderTimedOut)
            }
        }
    }
}

#[async_trait]
impl AssistantProviderProbeInterface for PythonOpenAiAssistantProviderAdapterImpl {
    async fn list_assistant_provider_models(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
    ) -> Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError> {
        match self.execute(ProviderControlOperation::ListModels, base_url, api_key).await? {
            ProviderControlOutput::Models(models) => Ok(models),
            ProviderControlOutput::Compatible => {
                Err(AssistantProviderProbeError::InvalidModelsResponse)
            }
        }
    }

    async fn test_assistant_provider_model(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
        model_id: &AssistantProviderModelId,
    ) -> Result<(), AssistantProviderProbeError> {
        match self.execute(ProviderControlOperation::TestModel(model_id), base_url, api_key).await?
        {
            ProviderControlOutput::Compatible => Ok(()),
            ProviderControlOutput::Models(_) => {
                Err(AssistantProviderProbeError::ResponsesEndpointUnavailable)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ProviderControlOperation<'a> {
    ListModels,
    TestModel(&'a AssistantProviderModelId),
}

impl<'a> ProviderControlOperation<'a> {
    const fn action(self) -> &'static str {
        match self {
            Self::ListModels => "list_models",
            Self::TestModel(_) => "test_model",
        }
    }

    const fn model_id(self) -> Option<&'a AssistantProviderModelId> {
        match self {
            Self::ListModels => None,
            Self::TestModel(model_id) => Some(model_id),
        }
    }
}

enum ProviderControlOutput {
    Models(Vec<AssistantProviderModelId>),
    Compatible,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProviderControlOutput {
    ok: bool,
    model_ids: Option<Vec<String>>,
    error: Option<String>,
}

fn decode_output(
    operation: ProviderControlOperation<'_>,
    encoded: &[u8],
) -> Result<ProviderControlOutput, AssistantProviderProbeError> {
    let output: RawProviderControlOutput =
        serde_json::from_slice(encoded).map_err(|_| malformed(operation))?;
    match (operation, output.ok, output.model_ids, output.error) {
        (ProviderControlOperation::ListModels, true, Some(model_ids), None) => {
            decode_models(model_ids)
        }
        (ProviderControlOperation::TestModel(_), true, None, None) => {
            Ok(ProviderControlOutput::Compatible)
        }
        (_, false, None, Some(error)) => Err(map_control_error(operation, &error)),
        _ => Err(malformed(operation)),
    }
}

fn decode_models(
    model_ids: Vec<String>,
) -> Result<ProviderControlOutput, AssistantProviderProbeError> {
    if model_ids.len() > MAX_PROVIDER_MODEL_COUNT {
        return Err(AssistantProviderProbeError::InvalidModelsResponse);
    }
    model_ids
        .into_iter()
        .map(|model_id| {
            AssistantProviderModelId::try_new(model_id)
                .map_err(|_| AssistantProviderProbeError::InvalidModelsResponse)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(ProviderControlOutput::Models)
}

fn map_control_error(
    operation: ProviderControlOperation<'_>,
    error: &str,
) -> AssistantProviderProbeError {
    match error {
        "authentication_rejected" => AssistantProviderProbeError::AuthenticationRejected,
        "provider_unreachable" => AssistantProviderProbeError::ProviderUnreachable,
        "provider_timed_out" => AssistantProviderProbeError::ProviderTimedOut,
        "models_endpoint_unavailable" => AssistantProviderProbeError::ModelsEndpointUnavailable,
        "invalid_models_response" => AssistantProviderProbeError::InvalidModelsResponse,
        "selected_model_rejected" => AssistantProviderProbeError::SelectedModelRejected,
        "responses_endpoint_unavailable" => {
            AssistantProviderProbeError::ResponsesEndpointUnavailable
        }
        "missing_function_tool_behavior" => {
            AssistantProviderProbeError::MissingFunctionToolBehavior
        }
        _ => malformed(operation),
    }
}

const fn malformed(operation: ProviderControlOperation<'_>) -> AssistantProviderProbeError {
    match operation {
        ProviderControlOperation::ListModels => AssistantProviderProbeError::InvalidModelsResponse,
        ProviderControlOperation::TestModel(_) => {
            AssistantProviderProbeError::ResponsesEndpointUnavailable
        }
    }
}

async fn terminate_child(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(test)]
#[path = "assistant_provider_probe_adapter_tests.rs"]
mod tests;
