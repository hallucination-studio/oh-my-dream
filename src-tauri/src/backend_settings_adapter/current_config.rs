use backends::generation_provider_settings::{
    GenerationProviderSettingsBinding, GenerationProviderSettingsError,
    GenerationProviderSettingsRevision, GenerationProviderSettingsSnapshot,
};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use serde::{Deserialize, Serialize};
use tasks::generation_task::{
    GenerationProviderId, GenerationProviderRouteId, GenerationTaskRequestKind,
};

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

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CurrentProviderBinding {
    profile_ref: String,
    generation_kind: String,
    provider_id: String,
    route_id: String,
}

pub(crate) fn encode(
    config: &DesktopBackendConfig,
) -> Result<Vec<u8>, DesktopBackendConfigRepositoryError> {
    encode_with_bindings(config, expected_mock_bindings())
}

pub(super) fn encode_with_bindings(
    config: &DesktopBackendConfig,
    generation_provider_routes: Vec<CurrentProviderBinding>,
) -> Result<Vec<u8>, DesktopBackendConfigRepositoryError> {
    serde_json::to_vec(&CurrentDesktopBackendConfig {
        sqlite_busy_timeout_ms: config.sqlite_busy_timeout_ms,
        post_commit_effect_concurrency: config.post_commit_effect_concurrency,
        workflow_run_concurrency: config.workflow_run_concurrency,
        workflow_node_concurrency: config.workflow_node_concurrency,
        generation_task_effect_concurrency: GENERATION_TASK_EFFECT_CONCURRENCY,
        asset_reconciliation_policy: config.asset_reconciliation_policy.clone(),
        asset_preview_policy: config.asset_preview_policy.clone(),
        generation_provider_routes,
        assistant_model: config.assistant_model.clone(),
        assistant_protocol_budgets: config.assistant_protocol_budgets.clone(),
    })
    .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)
}

pub(super) fn project(
    encoded: &[u8],
) -> Result<DesktopBackendConfig, DesktopBackendConfigRepositoryError> {
    let current = decode(encoded)?;
    let config = DesktopBackendConfig {
        sqlite_busy_timeout_ms: current.sqlite_busy_timeout_ms,
        post_commit_effect_concurrency: current.post_commit_effect_concurrency,
        workflow_run_concurrency: current.workflow_run_concurrency,
        workflow_node_concurrency: current.workflow_node_concurrency,
        generation_task_effect_concurrency: current.generation_task_effect_concurrency,
        asset_reconciliation_policy: current.asset_reconciliation_policy,
        asset_preview_policy: current.asset_preview_policy,
        generation_provider_routes: Vec::new(),
        assistant_model: current.assistant_model,
        assistant_protocol_budgets: current.assistant_protocol_budgets,
    };
    config.validate().map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    Ok(config)
}

pub(super) fn settings_snapshot(
    revision: i64,
    encoded: &[u8],
) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError> {
    let revision = u64::try_from(revision)
        .ok()
        .and_then(GenerationProviderSettingsRevision::new)
        .ok_or(GenerationProviderSettingsError::InvalidSnapshot)?;
    let current = decode(encoded).map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?;
    let bindings = current
        .generation_provider_routes
        .into_iter()
        .map(CurrentProviderBinding::into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    GenerationProviderSettingsSnapshot::try_new(revision, bindings)
}

pub(super) fn bindings(
    encoded: &[u8],
) -> Result<Vec<CurrentProviderBinding>, DesktopBackendConfigRepositoryError> {
    Ok(decode(encoded)?.generation_provider_routes)
}

impl CurrentProviderBinding {
    pub(super) fn from_domain(binding: &GenerationProviderSettingsBinding) -> Self {
        Self {
            profile_ref: binding.profile_ref().to_string(),
            generation_kind: kind_name(binding.generation_kind()).to_owned(),
            provider_id: binding.provider_id().as_str().to_owned(),
            route_id: binding.route_id().as_str().to_owned(),
        }
    }

    fn into_domain(
        self,
    ) -> Result<GenerationProviderSettingsBinding, GenerationProviderSettingsError> {
        let (profile_id, version) = self
            .profile_ref
            .split_once('@')
            .ok_or(GenerationProviderSettingsError::InvalidSnapshot)?;
        Ok(GenerationProviderSettingsBinding::new(
            GenerationProfileRef::new(
                GenerationProfileId::try_new(profile_id)
                    .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?,
                GenerationProfileVersion::try_new(
                    version
                        .parse()
                        .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?,
                )
                .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?,
            ),
            parse_kind(&self.generation_kind)?,
            GenerationProviderId::try_new(self.provider_id)
                .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?,
            GenerationProviderRouteId::try_new(self.route_id)
                .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?,
        ))
    }
}

fn decode(
    encoded: &[u8],
) -> Result<CurrentDesktopBackendConfig, DesktopBackendConfigRepositoryError> {
    let current: CurrentDesktopBackendConfig = serde_json::from_slice(encoded)
        .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    let canonical = serde_json::to_vec(&current)
        .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
    let routes_valid = current.generation_provider_routes.windows(2).all(|pair| pair[0] < pair[1])
        && current
            .generation_provider_routes
            .iter()
            .all(|binding| expected_mock_bindings().contains(binding));
    if canonical != encoded
        || current.generation_task_effect_concurrency != GENERATION_TASK_EFFECT_CONCURRENCY
        || !routes_valid
    {
        return Err(DesktopBackendConfigRepositoryError::InvalidConfig);
    }
    Ok(current)
}

fn parse_kind(value: &str) -> Result<GenerationTaskRequestKind, GenerationProviderSettingsError> {
    match value {
        "text" => Ok(GenerationTaskRequestKind::Text),
        "image" => Ok(GenerationTaskRequestKind::Image),
        "video" => Ok(GenerationTaskRequestKind::Video),
        "voice" => Ok(GenerationTaskRequestKind::Voice),
        _ => Err(GenerationProviderSettingsError::InvalidSnapshot),
    }
}

const fn kind_name(value: GenerationTaskRequestKind) -> &'static str {
    match value {
        GenerationTaskRequestKind::Text => "text",
        GenerationTaskRequestKind::Image => "image",
        GenerationTaskRequestKind::Video => "video",
        GenerationTaskRequestKind::Voice => "voice",
    }
}

fn expected_mock_bindings() -> Vec<CurrentProviderBinding> {
    [
        ("image.high_quality_general@1", "image", "mock.image.high-quality-general.v1"),
        ("speech.multilingual_narration@1", "voice", "mock.voice.multilingual-narration.v1"),
        ("video.cinematic_image_animation@1", "video", "mock.video.cinematic-image-animation.v1"),
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
