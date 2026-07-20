use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use rusqlite::Connection;

use crate::{
    assistant_process_command::AssistantSidecarCommand,
    assistant_provider_settings::{
        AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
        AssistantProviderModelsListUseCase, AssistantProviderProbeError,
        AssistantProviderProbeInterface, AssistantProviderSettingsError,
        AssistantProviderSettingsGetUseCase, AssistantProviderSettingsRevision,
        AssistantProviderSettingsTestAndApplyUseCase,
    },
    backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl,
};

use super::PythonOpenAiAssistantProviderAdapterImpl;

#[tokio::test]
async fn lists_models_through_secret_environment_and_strict_json() {
    let adapter = adapter(
        "import json,os; assert os.environ['OH_MY_DREAM_ASSISTANT_MODE']=='provider_control'; "
            .to_owned()
            + "assert os.environ['OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY']=='test-secret'; "
            + "print(json.dumps({'ok':True,'model_ids':['zeta','alpha']}))",
    );

    let models = adapter.list_assistant_provider_models(&base_url(), &api_key()).await.unwrap();

    assert_eq!(
        models.iter().map(AssistantProviderModelId::as_str).collect::<Vec<_>>(),
        vec!["zeta", "alpha"]
    );
}

#[tokio::test]
async fn maps_safe_structured_provider_failure_without_secret_text() {
    let adapter =
        adapter("print('{\"ok\":false,\"error\":\"authentication_rejected\"}')".to_owned());

    let result = adapter.list_assistant_provider_models(&base_url(), &api_key()).await;

    assert_eq!(result, Err(AssistantProviderProbeError::AuthenticationRejected));
    assert!(!format!("{result:?}").contains("test-secret"));
}

#[tokio::test]
async fn rejects_malformed_oversized_and_nonzero_process_output() {
    for script in [
        "print('not-json')".to_owned(),
        "print('{\"ok\":true,\"ok\":false,\"error\":\"provider_unreachable\"}')".to_owned(),
        "import json; print(json.dumps({'ok':True,'model_ids':['m']*10001}))".to_owned(),
        "import sys; sys.stdout.write('x' * 1048577)".to_owned(),
        "import sys; sys.exit(7)".to_owned(),
    ] {
        let result = adapter(script).list_assistant_provider_models(&base_url(), &api_key()).await;
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn times_out_and_rejects_an_unavailable_executable() {
    let timeout = PythonOpenAiAssistantProviderAdapterImpl::with_deadline(
        python_command("import time; time.sleep(5)".to_owned()),
        Duration::from_millis(25),
    );
    assert_eq!(
        timeout.list_assistant_provider_models(&base_url(), &api_key()).await,
        Err(AssistantProviderProbeError::ProviderTimedOut)
    );

    let unavailable = PythonOpenAiAssistantProviderAdapterImpl::new(AssistantSidecarCommand::new(
        "oh-my-dream-missing-provider-control-executable",
    ));
    assert_eq!(
        unavailable.list_assistant_provider_models(&base_url(), &api_key()).await,
        Err(AssistantProviderProbeError::ProviderUnreachable)
    );
}

#[tokio::test]
async fn tests_selected_model_through_the_control_mode() {
    let adapter = adapter(
        "import json,os; assert os.environ['OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION']=='test_model'; "
            .to_owned()
            + "assert os.environ['OH_MY_DREAM_ASSISTANT_PROVIDER_MODEL_ID']=='model-a'; "
            + "print(json.dumps({'ok':True}))",
    );

    adapter.test_assistant_provider_model(&base_url(), &api_key(), &model_id()).await.unwrap();
}

#[tokio::test]
async fn local_contract_server_proves_test_before_atomic_save_and_redaction() {
    let server = LocalContractServer::start();
    let adapter = Arc::new(PythonOpenAiAssistantProviderAdapterImpl::new(
        AssistantSidecarCommand::development(python(), repository_root()),
    ));
    let repository = Arc::new(
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::new(Mutex::new(
            Connection::open_in_memory().unwrap(),
        )))
        .unwrap(),
    );
    let base_url =
        AssistantProviderBaseUrl::try_new(format!("{}/valid/v1", server.origin)).unwrap();
    let models = AssistantProviderModelsListUseCase::new(adapter.clone(), repository.clone())
        .list_assistant_provider_models(base_url.clone(), Some(api_key()))
        .await
        .unwrap();
    assert_eq!(
        models.iter().map(AssistantProviderModelId::as_str).collect::<Vec<_>>(),
        ["model-a", "model-b"]
    );

    let apply = AssistantProviderSettingsTestAndApplyUseCase::new(adapter, repository.clone());
    let rejected = apply
        .test_and_apply_assistant_provider_settings(
            revision(1),
            base_url.clone(),
            Some(AssistantProviderApiKey::try_new(b"rejected-key".to_vec()).unwrap()),
            model_id(),
        )
        .await;
    assert_eq!(rejected, Err(AssistantProviderSettingsError::AuthenticationRejected));
    assert!(!format!("{rejected:?}").contains("rejected-key"));
    let unchanged = AssistantProviderSettingsGetUseCase::new(repository.clone())
        .get_assistant_provider_settings()
        .await
        .unwrap();
    assert_eq!(unchanged.settings_revision, revision(1));
    assert!(!unchanged.enabled);
    assert!(!unchanged.has_api_key);

    let saved = apply
        .test_and_apply_assistant_provider_settings(
            revision(1),
            base_url,
            Some(api_key()),
            model_id(),
        )
        .await
        .unwrap();
    assert_eq!(saved.settings_revision, revision(2));
    assert!(saved.enabled);
    assert!(saved.has_api_key);
}

fn adapter(script: String) -> PythonOpenAiAssistantProviderAdapterImpl {
    PythonOpenAiAssistantProviderAdapterImpl::new(python_command(script))
}

fn python_command(script: String) -> AssistantSidecarCommand {
    AssistantSidecarCommand::new(python()).args(["-c", &script])
}

fn python() -> std::ffi::OsString {
    std::env::var_os("OH_MY_DREAM_PYTHON").unwrap_or_else(|| "python3".into())
}

fn repository_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn revision(value: u64) -> AssistantProviderSettingsRevision {
    AssistantProviderSettingsRevision::new(value).unwrap()
}

fn base_url() -> AssistantProviderBaseUrl {
    AssistantProviderBaseUrl::try_new("http://127.0.0.1:11434/v1").unwrap()
}

fn model_id() -> AssistantProviderModelId {
    AssistantProviderModelId::try_new("model-a").unwrap()
}

fn api_key() -> AssistantProviderApiKey {
    AssistantProviderApiKey::try_new(b"test-secret".to_vec()).unwrap()
}

struct LocalContractServer {
    child: Child,
    origin: String,
}

impl LocalContractServer {
    fn start() -> Self {
        let mut child = Command::new(python())
            .args(["-m", "assistant.tests.openai_contract_server"])
            .current_dir(repository_root())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut origin = String::new();
        stdout.read_line(&mut origin).unwrap();
        assert!(origin.starts_with("http://127.0.0.1:"));
        Self { child, origin: origin.trim().to_owned() }
    }
}

impl Drop for LocalContractServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
