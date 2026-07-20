use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use assistant::interfaces::AssistantApplicationError;
use async_trait::async_trait;

use super::*;
use crate::assistant_model_runner::AssistantModelRuntimeConnection;
use crate::assistant_provider_settings::{
    AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
};

struct RuntimeConnectionReaderFakeImpl {
    connections: Mutex<VecDeque<(String, String, Vec<u8>)>>,
}

#[async_trait]
impl AssistantModelRuntimeConnectionReaderInterface for RuntimeConnectionReaderFakeImpl {
    async fn load_assistant_model_runtime_connection(
        &self,
    ) -> Result<AssistantModelRuntimeConnection, AssistantApplicationError> {
        let (base_url, model_id, api_key) = self
            .connections
            .lock()
            .map_err(|_| AssistantApplicationError::ModelUnavailable)?
            .pop_front()
            .ok_or(AssistantApplicationError::ModelUnavailable)?;
        Ok(AssistantModelRuntimeConnection::new(
            AssistantProviderBaseUrl::try_new(base_url)
                .map_err(|_| AssistantApplicationError::ModelUnavailable)?,
            AssistantProviderModelId::try_new(model_id)
                .map_err(|_| AssistantApplicationError::ModelUnavailable)?,
            AssistantProviderApiKey::try_new(api_key)
                .map_err(|_| AssistantApplicationError::ModelUnavailable)?,
        ))
    }
}

#[tokio::test]
async fn successive_launches_load_current_connection_and_running_child_keeps_old_values() {
    let reader = Arc::new(RuntimeConnectionReaderFakeImpl {
        connections: Mutex::new(VecDeque::from([
            ("http://old.test/v1".to_owned(), "model-old".to_owned(), b"key-old".to_vec()),
            ("https://new.test/v1".to_owned(), "model-new".to_owned(), b"key-new".to_vec()),
        ])),
    });
    let launcher =
        DynamicAssistantSidecarProcessLauncherAdapterImpl::new(environment_echo_command(), reader);

    let mut old_process = launcher.launch_assistant_protocol_process().await.unwrap();
    let mut new_process = launcher.launch_assistant_protocol_process().await.unwrap();
    let old = old_process.read_assistant_protocol_line().await.unwrap();
    let new = new_process.read_assistant_protocol_line().await.unwrap();

    assert_eq!(decode(&old), ["http://old.test/v1", "model-old", "key-old"]);
    assert_eq!(decode(&new), ["https://new.test/v1", "model-new", "key-new"]);
    old_process.abort_assistant_protocol_process().await;
    new_process.abort_assistant_protocol_process().await;
}

fn environment_echo_command() -> AssistantSidecarCommand {
    let script = "import json,os,sys; print(json.dumps([os.environ['OMD_ASSISTANT_BASE_URL'],os.environ['OMD_ASSISTANT_MODEL'],os.environ['OMD_ASSISTANT_API_KEY']])); sys.stdout.flush(); sys.stdin.buffer.read()";
    AssistantSidecarCommand::new(
        std::env::var_os("OH_MY_DREAM_PYTHON").unwrap_or_else(|| "python3".into()),
    )
    .args(["-c", script])
}

fn decode(encoded: &[u8]) -> [String; 3] {
    serde_json::from_slice(encoded).unwrap()
}
