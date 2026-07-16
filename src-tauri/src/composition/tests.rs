use std::sync::Mutex;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
    NodeCapabilityExecutionRequest, NodeCapabilityNormalizedParameters,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterError,
    NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
    WorkflowDataType, WorkflowNodeCapabilityInterface, WorkflowNodeOutputSet,
};
use tempfile::tempdir;

use super::*;
use crate::{
    credential_repository::{
        AssistantModelCredentialRepositoryInterface, AssistantModelCredentialSecret,
    },
    desktop_backend_config::DesktopBackendConfigRepositoryInterface,
    desktop_backend_config::GenerationProviderRouteConfig,
    post_commit_effect::DesktopPostCommitEffect,
    post_commit_worker::DesktopPostCommitEffectExecutionOutcome,
    workflow_run_event_publisher::DesktopEventEmissionError,
};

struct TestCapability {
    contract: NodeCapabilityContract,
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for TestCapability {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }

    async fn check_node_external_readiness(
        &self,
        _: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        Vec::new()
    }

    async fn execute_node_capability(
        &self,
        _: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        unreachable!("composition tests do not execute capabilities")
    }
}

struct TestExecutor;

#[async_trait]
impl DesktopPostCommitEffectExecutorInterface for TestExecutor {
    async fn execute_desktop_post_commit_effect(
        &self,
        _: DesktopPostCommitEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        DesktopPostCommitEffectExecutionOutcome::Completed
    }
}

struct TestInterrupter;

#[async_trait]
impl DesktopWorkflowRunRestartInterrupterInterface for TestInterrupter {
    async fn interrupt_workflow_run_after_restart(
        &self,
        _: engine::node_capability::WorkflowRunId,
    ) -> Result<(), engine::workflow::WorkflowApplicationError> {
        Ok(())
    }
}

#[derive(Default)]
struct TestEmitter {
    events: Mutex<Vec<String>>,
}

impl DesktopEventEmitterInterface for TestEmitter {
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
async fn missing_generation_credential_does_not_disable_assistant_or_host() {
    let directory = tempdir().expect("directory");
    let paths = DesktopApplicationPaths::from_application_data_root(directory.path());
    seed_config(&paths, true, true).await;

    let host = compose(paths).await.expect("host");

    assert!(host.assistant_commands_enabled());
    assert_eq!(host.config.generation_provider_routes.len(), 1);
}

async fn compose(
    paths: DesktopApplicationPaths,
) -> Result<DesktopApplicationHost, DesktopCompositionError> {
    DesktopCompositionRoot::compose_with_business(
        paths,
        Arc::new(TestEmitter::default()),
        |_| Ok(test_registry()),
        |_, _, _| {
            Ok(DesktopBusinessComposition {
                post_commit_effect_executor: Arc::new(TestExecutor),
                workflow_restart_interrupter: Arc::new(TestInterrupter),
            })
        },
    )
    .await
}

async fn seed_assistant_config(paths: &DesktopApplicationPaths, save_credential: bool) {
    seed_config(paths, save_credential, false).await;
}

async fn seed_config(
    paths: &DesktopApplicationPaths,
    save_assistant_credential: bool,
    include_generation_route_without_credential: bool,
) {
    std::fs::create_dir_all(&paths.config_root).expect("config root");
    let connection =
        open_metadata_sqlite(&metadata_sqlite_path(&paths.config_root)).expect("metadata");
    let settings =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::new(Mutex::new(connection)))
            .expect("settings");
    let mut config = DesktopBackendConfig::default();
    config.assistant_model.enabled = true;
    if include_generation_route_without_credential {
        config.generation_provider_routes.push(GenerationProviderRouteConfig {
            profile_ref: "image.high_quality_general@1".to_owned(),
            route_id: "fal.text_to_image".to_owned(),
            account_id: "fal.default".to_owned(),
            endpoint: "https://queue.fal.run/fal-ai/flux-pro/kontext/text-to-image".to_owned(),
            native_model_id: "fal-ai/flux-pro/kontext/text-to-image".to_owned(),
            credential_id: "fal.missing".to_owned(),
            operation_deadline_ms: 180_000,
            poll_min_delay_ms: 500,
            poll_max_delay_ms: 5_000,
            download_host_allowlist: vec!["v3.fal.media".to_owned()],
        });
    }
    settings.save_desktop_backend_config(config.clone()).await.expect("save config");
    if save_assistant_credential {
        settings
            .save_assistant_model_credential(
                AssistantModelCredentialId::new(config.assistant_model.credential_id)
                    .expect("credential ID"),
                AssistantModelCredentialSecret::new(b"plain-test-secret".to_vec()).expect("secret"),
            )
            .await
            .expect("save credential");
    }
}

fn test_registry() -> Arc<WorkflowNodeCapabilityRegistry> {
    let capabilities = EXACT_NODE_CAPABILITY_REFS
        .into_iter()
        .map(|reference| {
            Arc::new(TestCapability { contract: test_contract(reference) })
                as Arc<dyn WorkflowNodeCapabilityInterface>
        })
        .collect::<Vec<_>>();
    Arc::new(WorkflowNodeCapabilityRegistry::try_new(capabilities).expect("registry"))
}

fn test_contract(reference: &str) -> NodeCapabilityContract {
    let (id, version) = reference.split_once('@').expect("reference");
    let (major, minor) = version.split_once('.').expect("version");
    let contract_ref = NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).expect("ID"),
        NodeCapabilityContractVersion::new(
            major.parse().expect("major"),
            minor.parse().expect("minor"),
        )
        .expect("version"),
    );
    NodeCapabilityContract::try_new(
        contract_ref,
        Vec::new(),
        Vec::new(),
        vec![NodeCapabilityOutputContract::new(
            NodeCapabilityOutputKey::new("result").expect("output"),
            WorkflowDataType::Text,
            true,
        )],
        NodeCapabilityExecutionKind::PureValue,
    )
    .expect("contract")
}
