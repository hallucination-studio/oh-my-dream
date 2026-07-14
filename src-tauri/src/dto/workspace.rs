//! Bounded API DTOs for the authoritative workspace snapshot operation.

use super::{ProjectDto, WorkflowHeadDto, asset_kind_as_str};
use crate::dto::CapabilityRefDto;
use assets::Asset;
use engine::WorkflowReadinessBlocker;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_WORKSPACE_PROMPT_CHARS: usize = 512;

/// Empty model input for a workspace read.
///
/// Project and selection scope are supplied by the trusted Rust invocation
/// context, so the model cannot change the current UI selection.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshotInput {}

/// Trusted invocation scope echoed in the snapshot result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceScopeDto {
    /// Project identity supplied outside model input.
    pub project_id: String,
    /// Assistant session identity supplied outside model input.
    pub session_id: String,
    /// Idempotent request identity supplied outside model input.
    pub request_id: String,
}

/// Bounded Asset metadata that omits local paths and Workflow snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceAssetSummaryDto {
    /// Stable Asset identity.
    pub id: String,
    /// `image`, `video`, or `audio`.
    pub kind: String,
    /// Owning Project, or null for a global local Asset.
    pub project_id: Option<String>,
    /// Source Workflow node, when known.
    pub source_node_id: Option<String>,
    /// Source exact capability id, when known.
    pub source_node_type: Option<String>,
    /// Provider model identifier, when known.
    pub model: Option<String>,
    /// Prompt text bounded for Agent context.
    pub prompt: Option<String>,
    /// Whether prompt text was shortened at the API boundary.
    pub prompt_truncated: bool,
    /// Creation time as Unix seconds.
    pub created_at: i64,
}

impl From<Asset> for WorkspaceAssetSummaryDto {
    fn from(asset: Asset) -> Self {
        let (prompt, prompt_truncated) = bounded_prompt(asset.prompt);
        Self {
            id: asset.id,
            kind: asset_kind_as_str(asset.kind).to_owned(),
            project_id: asset.project_id,
            source_node_id: asset.source_node_id,
            source_node_type: asset.source_node_type,
            model: asset.model,
            prompt,
            prompt_truncated,
            created_at: asset.created_at,
        }
    }
}

/// Selected Workflow node identity without another copy of its params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceNodeSummaryDto {
    /// Stable node id in the current Workflow head.
    pub id: String,
    /// Persisted exact capability identity.
    pub capability: CapabilityRefDto,
}

/// Bounded project-scoped Run state available before durable Run history lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRunSummaryDto {
    /// Caller-provided Run identity.
    pub run_id: String,
    /// Current state; Task 13 exposes only active in-memory Runs.
    pub status: String,
}

/// One authoritative, bounded read of the current Project workspace.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshotOutput {
    /// Trusted invocation scope.
    pub scope: WorkspaceScopeDto,
    /// Current Project metadata.
    pub project: ProjectDto,
    /// Current authoritative head, or null when no mutation has created one.
    pub workflow_head: Option<WorkflowHeadDto>,
    /// Selected Assets resolved within the trusted Project scope.
    pub selected_assets: Vec<WorkspaceAssetSummaryDto>,
    /// Selected nodes resolved from the current authoritative head.
    pub selected_nodes: Vec<WorkspaceNodeSummaryDto>,
    /// Persistable but non-runnable Workflow conditions.
    pub readiness_blockers: Vec<WorkflowReadinessBlocker>,
    /// Bounded newest-first Project Asset summaries.
    pub assets: Vec<WorkspaceAssetSummaryDto>,
    /// Bounded current Project Run summaries.
    pub runs: Vec<WorkspaceRunSummaryDto>,
}

fn bounded_prompt(prompt: Option<String>) -> (Option<String>, bool) {
    let Some(prompt) = prompt else {
        return (None, false);
    };
    let mut characters = prompt.chars();
    let bounded = characters.by_ref().take(MAX_WORKSPACE_PROMPT_CHARS).collect::<String>();
    let truncated = characters.next().is_some();
    (Some(bounded), truncated)
}

#[cfg(test)]
mod tests {
    use super::bounded_prompt;

    #[test]
    fn prompt_summary_is_bounded_by_unicode_characters() {
        let prompt = "x".repeat(513);
        let (bounded, truncated) = bounded_prompt(Some(prompt));
        assert_eq!(bounded.expect("prompt").chars().count(), 512);
        assert!(truncated);
    }
}
