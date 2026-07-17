use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::json::decode_strict_json;

const MAX_CONFIG_JSON_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopBackendConfigError {
    #[error("Desktop backend configuration is invalid")]
    InvalidConfig,
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopBackendConfigRepositoryError {
    #[error("Desktop backend configuration is invalid")]
    InvalidConfig,
    #[error("Desktop backend configuration storage permission denied")]
    PermissionDenied,
    #[error("Desktop backend configuration storage unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopBackendConfig {
    pub sqlite_busy_timeout_ms: u64,
    pub post_commit_effect_concurrency: u8,
    pub workflow_run_concurrency: u8,
    pub workflow_node_concurrency: u8,
    pub generation_task_effect_concurrency: u8,
    pub asset_reconciliation_policy: AssetReconciliationPolicy,
    pub asset_preview_policy: AssetPreviewPolicy,
    pub generation_provider_routes: Vec<GenerationProviderRouteConfig>,
    pub assistant_model: AssistantModelConfig,
    pub assistant_protocol_budgets: AssistantProtocolBudgets,
}

impl Default for DesktopBackendConfig {
    fn default() -> Self {
        Self {
            sqlite_busy_timeout_ms: 5_000,
            post_commit_effect_concurrency: 4,
            workflow_run_concurrency: 1,
            workflow_node_concurrency: 2,
            generation_task_effect_concurrency: 4,
            asset_reconciliation_policy: AssetReconciliationPolicy::default(),
            asset_preview_policy: AssetPreviewPolicy::default(),
            generation_provider_routes: Vec::new(),
            assistant_model: AssistantModelConfig::default(),
            assistant_protocol_budgets: AssistantProtocolBudgets::default(),
        }
    }
}

impl DesktopBackendConfig {
    pub fn from_canonical_json(encoded: &[u8]) -> Result<Self, DesktopBackendConfigError> {
        if encoded.is_empty() || encoded.len() > MAX_CONFIG_JSON_BYTES {
            return Err(DesktopBackendConfigError::InvalidConfig);
        }
        let value = decode_strict_json(encoded)?;
        let config: Self =
            serde_json::from_value(value).map_err(|_| DesktopBackendConfigError::InvalidConfig)?;
        config.validate()?;
        if config.canonical_json()?.as_slice() != encoded {
            return Err(DesktopBackendConfigError::InvalidConfig);
        }
        Ok(config)
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, DesktopBackendConfigError> {
        self.validate()?;
        let encoded =
            serde_json::to_vec(self).map_err(|_| DesktopBackendConfigError::InvalidConfig)?;
        if encoded.is_empty() || encoded.len() > MAX_CONFIG_JSON_BYTES {
            Err(DesktopBackendConfigError::InvalidConfig)
        } else {
            Ok(encoded)
        }
    }

    pub fn validate(&self) -> Result<(), DesktopBackendConfigError> {
        let concurrency = [
            self.post_commit_effect_concurrency,
            self.workflow_run_concurrency,
            self.workflow_node_concurrency,
            self.generation_task_effect_concurrency,
        ];
        let route_keys = self
            .generation_provider_routes
            .iter()
            .map(|route| (&route.profile_ref, &route.route_id))
            .collect::<BTreeSet<_>>();
        let valid = self.sqlite_busy_timeout_ms == 5_000
            && concurrency.into_iter().all(|value| (1..=8).contains(&value))
            && self.asset_reconciliation_policy.is_valid()
            && self.asset_preview_policy.is_valid()
            && route_keys.len() == self.generation_provider_routes.len()
            && self.generation_provider_routes.iter().all(GenerationProviderRouteConfig::is_valid)
            && self.assistant_model.is_valid()
            && self.assistant_protocol_budgets.is_valid();
        if valid { Ok(()) } else { Err(DesktopBackendConfigError::InvalidConfig) }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReconciliationPolicy {
    pub page_limit: u8,
    pub operation_deadline_ms: u64,
    pub stale_staging_after_ms: u64,
}

impl Default for AssetReconciliationPolicy {
    fn default() -> Self {
        Self { page_limit: 50, operation_deadline_ms: 30_000, stale_staging_after_ms: 86_400_000 }
    }
}

impl AssetReconciliationPolicy {
    fn is_valid(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetPreviewPolicy {
    pub lease_lifetime_ms: u64,
    pub max_range_bytes: u64,
}

impl Default for AssetPreviewPolicy {
    fn default() -> Self {
        Self { lease_lifetime_ms: 300_000, max_range_bytes: 16_777_216 }
    }
}

impl AssetPreviewPolicy {
    fn is_valid(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProviderRouteConfig {
    pub profile_ref: String,
    pub route_id: String,
    pub account_id: String,
    pub endpoint: String,
    pub native_model_id: String,
    pub credential_id: String,
    pub operation_deadline_ms: u64,
    pub poll_min_delay_ms: u64,
    pub poll_max_delay_ms: u64,
    pub download_host_allowlist: Vec<String>,
}

impl GenerationProviderRouteConfig {
    fn is_valid(&self) -> bool {
        let hosts = self.download_host_allowlist.iter().collect::<BTreeSet<_>>();
        let hosts_valid = hosts.len() == self.download_host_allowlist.len()
            && self.download_host_allowlist.windows(2).all(|pair| pair[0] < pair[1])
            && self.download_host_allowlist.iter().all(|host| valid_host(host));
        valid_identity(&self.account_id)
            && valid_identity(&self.credential_id)
            && hosts_valid
            && self.matches_frozen_route()
    }

    fn matches_frozen_route(&self) -> bool {
        match (self.profile_ref.as_str(), self.route_id.as_str()) {
            ("image.high_quality_general@1", "fal.text_to_image") => {
                self.account_id.starts_with("fal.")
                    && !self.download_host_allowlist.is_empty()
                    && self.endpoint
                        == "https://queue.fal.run/fal-ai/flux-pro/kontext/text-to-image"
                    && self.native_model_id == "fal-ai/flux-pro/kontext/text-to-image"
                    && self.operation_deadline_ms == 180_000
                    && self.poll_min_delay_ms == 500
                    && self.poll_max_delay_ms == 5_000
            }
            ("video.cinematic_image_animation@1", "fal.image_to_video") => {
                self.account_id.starts_with("fal.")
                    && !self.download_host_allowlist.is_empty()
                    && self.endpoint
                        == "https://queue.fal.run/fal-ai/kling-video/v3/standard/image-to-video"
                    && self.native_model_id == "fal-ai/kling-video/v3/standard/image-to-video"
                    && self.operation_deadline_ms == 900_000
                    && self.poll_min_delay_ms == 500
                    && self.poll_max_delay_ms == 5_000
            }
            ("speech.multilingual_narration@1", "elevenlabs.text_to_speech") => {
                self.account_id.starts_with("elevenlabs.")
                    && valid_elevenlabs_endpoint(&self.endpoint)
                    && self.native_model_id == "eleven_multilingual_v2"
                    && self.operation_deadline_ms == 120_000
                    && self.poll_min_delay_ms == 0
                    && self.poll_max_delay_ms == 0
                    && self.download_host_allowlist.is_empty()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantModelConfig {
    pub schema_version: u32,
    pub enabled: bool,
    pub model_profile_ref: String,
    pub credential_id: String,
}

impl Default for AssistantModelConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled: false,
            model_profile_ref: "assistant.workflow_coauthor@1".to_owned(),
            credential_id: "assistant.openai.default".to_owned(),
        }
    }
}

impl AssistantModelConfig {
    fn is_valid(&self) -> bool {
        self.schema_version == 1
            && self.model_profile_ref == "assistant.workflow_coauthor@1"
            && valid_identity(&self.credential_id)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantProtocolBudgets {
    pub invocation_deadline_ms: u64,
    pub frame_max_bytes: usize,
    pub json_max_depth: usize,
    pub event_max_count: usize,
    pub tool_call_max_count: usize,
    pub model_turn_max_count: usize,
    pub direction_max_bytes: usize,
    pub text_output_max_bytes: usize,
    pub snapshot_max_bytes: usize,
    pub candidate_max_bytes: usize,
    pub continuation_max_bytes: usize,
    pub approval_expiry_ms: u64,
}

impl Default for AssistantProtocolBudgets {
    fn default() -> Self {
        Self {
            invocation_deadline_ms: 600_000,
            frame_max_bytes: 8 * 1024 * 1024,
            json_max_depth: 32,
            event_max_count: 512,
            tool_call_max_count: 64,
            model_turn_max_count: 16,
            direction_max_bytes: 16 * 1024 * 1024,
            text_output_max_bytes: 16 * 1024,
            snapshot_max_bytes: 1024 * 1024,
            candidate_max_bytes: 1024 * 1024,
            continuation_max_bytes: 4 * 1024 * 1024,
            approval_expiry_ms: 30 * 60 * 1_000,
        }
    }
}

impl AssistantProtocolBudgets {
    fn is_valid(&self) -> bool {
        self == &Self::default()
    }
}

fn valid_identity(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|first| first.is_ascii_lowercase())
                && chars.all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        })
}

fn valid_host(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
}

fn valid_elevenlabs_endpoint(value: &str) -> bool {
    let prefix = "https://api.elevenlabs.io/v1/text-to-speech/";
    let suffix = "?output_format=mp3_44100_128";
    value.strip_prefix(prefix).and_then(|value| value.strip_suffix(suffix)).is_some_and(|voice| {
        (1..=128).contains(&voice.len())
            && voice
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    })
}
