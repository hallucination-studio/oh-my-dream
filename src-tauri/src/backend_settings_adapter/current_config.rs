use serde::{Deserialize, Serialize};

use crate::desktop_backend_config::{
    AssetPreviewPolicy, AssetReconciliationPolicy, AssistantModelConfig, AssistantProtocolBudgets,
    DesktopBackendConfig, DesktopBackendConfigRepositoryError,
};

const GENERATION_TASK_EFFECT_CONCURRENCY: u8 = 4;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CurrentDesktopBackendConfig {
    sqlite_busy_timeout_ms: u64,
    post_commit_effect_concurrency: u8,
    workflow_run_concurrency: u8,
    workflow_node_concurrency: u8,
    generation_task_effect_concurrency: u8,
    asset_reconciliation_policy: AssetReconciliationPolicy,
    asset_preview_policy: AssetPreviewPolicy,
    generation_provider_routes: Vec<CurrentProviderBinding>,
    assistant_model: AssistantModelConfig,
    assistant_protocol_budgets: AssistantProtocolBudgets,
}

#[derive(Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct CurrentProviderBinding {
    profile_ref: String,
    generation_kind: String,
    provider_id: String,
    route_id: String,
}

pub(crate) fn encode(
    legacy: &DesktopBackendConfig,
) -> Result<Vec<u8>, DesktopBackendConfigRepositoryError> {
    serde_json::to_vec(&CurrentDesktopBackendConfig {
        sqlite_busy_timeout_ms: legacy.sqlite_busy_timeout_ms,
        post_commit_effect_concurrency: legacy.post_commit_effect_concurrency,
        workflow_run_concurrency: legacy.workflow_run_concurrency,
        workflow_node_concurrency: legacy.workflow_node_concurrency,
        generation_task_effect_concurrency: GENERATION_TASK_EFFECT_CONCURRENCY,
        asset_reconciliation_policy: legacy.asset_reconciliation_policy.clone(),
        asset_preview_policy: legacy.asset_preview_policy.clone(),
        generation_provider_routes: expected_mock_bindings(),
        assistant_model: legacy.assistant_model.clone(),
        assistant_protocol_budgets: legacy.assistant_protocol_budgets.clone(),
    })
    .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)
}

pub(super) fn project(
    encoded: &[u8],
) -> Result<DesktopBackendConfig, DesktopBackendConfigRepositoryError> {
    let current: CurrentDesktopBackendConfig = serde_json::from_slice(encoded)
        .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    let canonical = serde_json::to_vec(&current)
        .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    if canonical != encoded
        || current.generation_task_effect_concurrency != GENERATION_TASK_EFFECT_CONCURRENCY
        || current.generation_provider_routes != expected_mock_bindings()
    {
        return Err(DesktopBackendConfigRepositoryError::InvalidConfig);
    }
    let config = DesktopBackendConfig {
        sqlite_busy_timeout_ms: current.sqlite_busy_timeout_ms,
        post_commit_effect_concurrency: current.post_commit_effect_concurrency,
        workflow_run_concurrency: current.workflow_run_concurrency,
        workflow_node_concurrency: current.workflow_node_concurrency,
        asset_reconciliation_policy: current.asset_reconciliation_policy,
        asset_preview_policy: current.asset_preview_policy,
        generation_provider_routes: Vec::new(),
        assistant_model: current.assistant_model,
        assistant_protocol_budgets: current.assistant_protocol_budgets,
    };
    config.validate().map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    Ok(config)
}

fn expected_mock_bindings() -> Vec<CurrentProviderBinding> {
    [
        ("image.high_quality_general@1", "image", "mock.image.high-quality-general.v1"),
        ("video.cinematic_image_animation@1", "video", "mock.video.cinematic-image-animation.v1"),
        ("speech.multilingual_narration@1", "voice", "mock.voice.multilingual-narration.v1"),
    ]
    .into_iter()
    .map(|(profile_ref, generation_kind, route_id)| CurrentProviderBinding {
        profile_ref: profile_ref.to_owned(),
        generation_kind: generation_kind.to_owned(),
        provider_id: "mock".to_owned(),
        route_id: route_id.to_owned(),
    })
    .collect()
}
