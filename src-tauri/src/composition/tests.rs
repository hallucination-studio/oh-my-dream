use std::sync::Mutex;

use async_trait::async_trait;
use tempfile::tempdir;

use super::*;
use crate::{
    assistant_provider_settings::{
        AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
        AssistantProviderSettingsMutation, AssistantProviderSettingsRepositoryInterface,
    },
    credential_repository::AssistantModelCredentialRepositoryInterface,
    post_commit_effect::DesktopPostCommitEffect,
    post_commit_worker::DesktopPostCommitEffectExecutionOutcome,
    workflow_run_event_publisher::DesktopEventEmissionError,
};

struct TestExecutorImpl;

#[async_trait]
impl DesktopPostCommitEffectExecutorInterface for TestExecutorImpl {
    async fn execute_desktop_post_commit_effect(
        &self,
        _: DesktopPostCommitEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        DesktopPostCommitEffectExecutionOutcome::Completed
    }
}

struct TestInterrupterImpl;

#[async_trait]
impl DesktopWorkflowRunRestartInterrupterInterface for TestInterrupterImpl {
    async fn interrupt_workflow_run_after_restart(
        &self,
        _: engine::node_capability::WorkflowRunId,
    ) -> Result<(), engine::workflow::WorkflowApplicationError> {
        Ok(())
    }
}

#[derive(Default)]
struct TestEmitterImpl {
    events: Mutex<Vec<String>>,
}

impl DesktopEventEmitterInterface for TestEmitterImpl {
    fn emit_desktop_event(
        &self,
        event_name: &str,
        _: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        self.events.lock().map_err(|_| DesktopEventEmissionError)?.push(event_name.to_owned());
        Ok(())
    }
}

#[tokio::test]
async fn missing_assistant_credential_disables_only_assistant_commands() {
    let directory = tempdir().expect("directory");
    let paths = DesktopApplicationPaths::from_application_data_root(directory.path());
    seed_assistant_config(&paths, false).await;

    let host = compose(paths).await.expect("host");

    assert!(!host.assistant_commands_enabled());
    assert_eq!(host.node_capabilities.list_node_capability_contracts().len(), 7);
    assert_eq!(host.config.post_commit_effect_concurrency, 4);
}

#[tokio::test]
async fn present_assistant_credential_enables_assistant_commands_without_retaining_secret() {
    let directory = tempdir().expect("directory");
    let paths = DesktopApplicationPaths::from_application_data_root(directory.path());
    seed_assistant_config(&paths, true).await;

    let host = compose(paths).await.expect("host");

    assert!(host.assistant_commands_enabled());
}

#[tokio::test]
async fn mock_generation_routes_do_not_require_credentials() {
    let directory = tempdir().expect("directory");
    let paths = DesktopApplicationPaths::from_application_data_root(directory.path());
    seed_config(&paths, true).await;

    let host = compose(paths).await.expect("host");

    assert!(host.assistant_commands_enabled());
    assert_eq!(host.config.generation_provider_routes.len(), 3);
}

#[tokio::test]
async fn activated_commands_expose_only_the_frozen_mock_provider_contract() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("activated commands");

    assert_eq!(dependencies.generation_task_provider_contracts.len(), 1);
    let contract = &dependencies.generation_task_provider_contracts[0];
    assert_eq!(contract.provider_id().as_str(), "mock");
    assert_eq!(contract.display_name().as_str(), "Mock");
    assert!(contract.text().is_none());
    assert_eq!(contract.image().expect("image capability").routes().len(), 1);
    assert_eq!(contract.video().expect("video capability").routes().len(), 1);
    assert_eq!(contract.voice().expect("voice capability").routes().len(), 1);
    assert_eq!(
        contract.image().unwrap().routes()[0].route_id().as_str(),
        "mock.image.high-quality-general.v1"
    );
    assert_eq!(
        contract.image().unwrap().routes()[0]
            .compatible_generation_profiles()
            .iter()
            .next()
            .unwrap()
            .to_string(),
        "image.high_quality_general@1"
    );
    assert_eq!(
        contract.video().unwrap().routes()[0].route_id().as_str(),
        "mock.video.cinematic-image-animation.v1"
    );
    assert_eq!(
        contract.video().unwrap().routes()[0]
            .compatible_generation_profiles()
            .iter()
            .next()
            .unwrap()
            .to_string(),
        "video.cinematic_image_animation@1"
    );
    assert_eq!(
        contract.voice().unwrap().routes()[0].route_id().as_str(),
        "mock.voice.multilingual-narration.v1"
    );
    assert_eq!(
        contract.voice().unwrap().routes()[0]
            .compatible_generation_profiles()
            .iter()
            .next()
            .unwrap()
            .to_string(),
        "speech.multilingual_narration@1"
    );
    assert_eq!(dependencies.node_capability_list.list_node_capabilities().len(), 7);
}

async fn compose(
    paths: DesktopApplicationPaths,
) -> Result<DesktopApplicationHost, DesktopCompositionError> {
    DesktopCompositionRoot::compose_with_business(
        paths,
        Arc::new(TestEmitterImpl::default()),
        |_, _, _| {
            Ok(DesktopBusinessComposition {
                post_commit_effect_executor: Arc::new(TestExecutorImpl),
                workflow_restart_interrupter: Arc::new(TestInterrupterImpl),
            })
        },
    )
    .await
}

async fn seed_assistant_config(paths: &DesktopApplicationPaths, save_credential: bool) {
    seed_config(paths, save_credential).await;
}

async fn seed_config(paths: &DesktopApplicationPaths, save_assistant_credential: bool) {
    std::fs::create_dir_all(&paths.config_root).expect("config root");
    let connection =
        open_metadata_sqlite(&metadata_sqlite_path(&paths.config_root)).expect("metadata");
    let settings =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::new(Mutex::new(connection)))
            .expect("settings");
    let initial = settings
        .load_assistant_provider_settings_snapshot()
        .await
        .expect("load Assistant settings");
    settings
        .apply_assistant_provider_settings_mutation(
            initial.revision(),
            AssistantProviderSettingsMutation::ApplyTestedConnection {
                base_url: AssistantProviderBaseUrl::try_new("https://api.openai.com/v1")
                    .expect("Base URL"),
                model_id: AssistantProviderModelId::try_new("fixture-model").expect("model ID"),
                api_key: Some(
                    AssistantProviderApiKey::try_new(b"plain-test-secret".to_vec())
                        .expect("API key"),
                ),
            },
        )
        .await
        .expect("apply Assistant settings");
    if !save_assistant_credential {
        settings
            .delete_assistant_model_credential(
                &AssistantModelCredentialId::new("assistant.openai.default")
                    .expect("credential ID"),
            )
            .await
            .expect("remove credential");
    }
}
