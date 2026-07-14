use crate::workflow_authority::WorkflowHead;
use assets::{Asset, AssetKind, Project};
use engine::{NodeExecutionState, NodeProgressEvent, RunOutputs, Value, ValueMap};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod capability;
mod workspace;

pub use capability::{
    CapabilityAvailabilityDto, CapabilityBundleDto, CapabilityBundlesDto, CapabilityCardinalityDto,
    CapabilityCatalogDto, CapabilityCatalogEntryDto, CapabilityContractDto, CapabilityEffectDto,
    CapabilityPortDto, CapabilityPresentationDto, CapabilityRefDto, CapabilitySearchPageDto,
    CapabilitySelectorDto, CapabilityStatusDto, CapabilitySummaryDto,
};
pub(crate) use workspace::MAX_WORKSPACE_PROMPT_CHARS;
pub use workspace::{
    WorkspaceAssetSummaryDto, WorkspaceNodeSummaryDto, WorkspaceRunSummaryDto, WorkspaceScopeDto,
    WorkspaceSnapshotInput, WorkspaceSnapshotOutput,
};

/// JSON-friendly result returned by the `run_workflow` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowResultDto {
    /// Node id -> output name -> output value.
    pub outputs: BTreeMap<String, BTreeMap<String, RunOutputDto>>,
}

impl RunWorkflowResultDto {
    /// Converts engine outputs into the frontend run-output shape.
    #[must_use]
    pub fn from_outputs(outputs: &RunOutputs) -> Self {
        Self {
            outputs: outputs
                .iter()
                .map(|(node_id, values)| (node_id.clone(), value_map_to_dto(values)))
                .collect(),
        }
    }
}

/// A single output value as consumed by the frontend seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunOutputDto {
    /// Engine value kind preserved for the frontend contract.
    pub kind: String,
    /// String representation of the produced value.
    pub value: String,
}

/// Stored asset metadata as returned by Tauri commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDto {
    /// Unique asset id.
    pub id: String,
    /// `image`, `video`, or `audio`.
    pub kind: String,
    /// Stored media path.
    pub file_path: String,
    /// Stored thumbnail path, when available.
    pub thumbnail_path: Option<String>,
    /// Workflow snapshot captured when the asset was saved.
    pub workflow_snapshot: serde_json::Value,
    /// Prompt text that ultimately produced this asset, when available.
    pub prompt: Option<String>,
    /// Project id this asset belongs to.
    pub project_id: Option<String>,
    /// Project name captured for display.
    pub project_name: Option<String>,
    /// Source workflow node id.
    pub source_node_id: Option<String>,
    /// Source workflow node type.
    pub source_node_type: Option<String>,
    /// Model identifier used to produce this asset.
    pub model: Option<String>,
    /// Seed used to produce this asset, encoded as decimal text for JavaScript safety.
    pub seed: Option<String>,
    /// Estimated cost in micro-USD.
    pub cost: Option<i64>,
    /// Free-form asset tags.
    pub tags: Vec<String>,
    /// Unix timestamp in seconds.
    pub created_at: i64,
}

impl From<Asset> for AssetDto {
    fn from(asset: Asset) -> Self {
        Self {
            id: asset.id,
            kind: asset_kind_as_str(asset.kind).to_owned(),
            file_path: asset.file_path,
            thumbnail_path: asset.thumbnail_path,
            workflow_snapshot: asset.workflow_snapshot,
            prompt: asset.prompt,
            project_id: asset.project_id,
            project_name: asset.project_name,
            source_node_id: asset.source_node_id,
            source_node_type: asset.source_node_type,
            model: asset.model,
            seed: asset.seed.map(|seed| seed.to_string()),
            cost: asset.cost,
            tags: asset.tags,
            created_at: asset.created_at,
        }
    }
}

/// Project metadata returned by project commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDto {
    /// Unique project id.
    pub id: String,
    /// User-visible project name.
    pub name: String,
    /// Unix timestamp in seconds.
    pub created_at: i64,
}

impl From<Project> for ProjectDto {
    fn from(project: Project) -> Self {
        Self { id: project.id, name: project.name, created_at: project.created_at }
    }
}

/// Authoritative Workflow head returned when a Project is opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowHeadDto {
    /// Project scope for this head.
    pub project_id: String,
    /// Compare-and-swap revision.
    pub revision: u64,
    /// Portable Workflow document.
    pub workflow: serde_json::Value,
}

impl TryFrom<WorkflowHead> for WorkflowHeadDto {
    type Error = serde_json::Error;

    fn try_from(head: WorkflowHead) -> Result<Self, Self::Error> {
        Ok(Self {
            project_id: head.project_id,
            revision: head.revision,
            workflow: serde_json::to_value(head.workflow)?,
        })
    }
}

/// Project plus its optional authoritative Workflow head.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenProjectResultDto {
    /// Project metadata.
    pub project: ProjectDto,
    /// `None` means the Project has never accepted a non-empty Workflow mutation.
    pub workflow_head: Option<WorkflowHeadDto>,
}

/// Compatibility name for callers that still refer to an opened Project workspace.
pub type ProjectWorkspaceDto = OpenProjectResultDto;

/// Provider configuration summary returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDto {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Whether this provider is active.
    pub active: bool,
    /// Whether a local key exists. Raw keys are never returned.
    pub has_key: bool,
}

/// Assistant configuration summary returned to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantConfigDto {
    /// Whether the assistant is enabled.
    pub enabled: bool,
    /// OpenAI-compatible base URL.
    pub base_url: String,
    /// Chat model identifier.
    pub model: String,
    /// Whether a local API key exists. Raw keys are never returned.
    pub has_key: bool,
}

/// Assistant configuration input accepted from the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantConfigInputDto {
    /// Whether the assistant is enabled.
    pub enabled: bool,
    /// OpenAI-compatible base URL.
    pub base_url: String,
    /// Chat model identifier.
    pub model: String,
    /// New API key. `None` preserves the stored key unless `clear_api_key` is true.
    pub api_key: Option<String>,
    /// Whether to remove the stored API key.
    pub clear_api_key: bool,
}

/// Node progress event forwarded to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeProgressEventDto {
    /// Workflow node id.
    pub node_id: String,
    /// `idle`, `running`, `done`, `cached`, or `error`.
    pub state: String,
    /// Best-effort progress in `[0.0, 1.0]`.
    pub progress: Option<f32>,
    /// Estimated cost in micro-USD.
    pub cost: Option<i64>,
}

impl From<NodeProgressEvent> for NodeProgressEventDto {
    fn from(event: NodeProgressEvent) -> Self {
        Self {
            node_id: event.node_id,
            state: node_state_as_str(event.state).to_owned(),
            progress: event.progress,
            cost: event.cost,
        }
    }
}

fn value_map_to_dto(values: &ValueMap) -> BTreeMap<String, RunOutputDto> {
    values.iter().map(|(name, value)| (name.clone(), run_output_to_dto(value))).collect()
}

fn run_output_to_dto(value: &Value) -> RunOutputDto {
    match value {
        Value::String(value) => output("string", value),
        Value::Image(value) => output("image", value),
        Value::Video(value) => output("video", value),
        Value::Audio(value) => output("audio", value),
        Value::Model(value) => output("model", value),
        Value::Int(value) => output("int", &value.to_string()),
        Value::Float(value) => output("float", &value.to_string()),
    }
}

fn output(kind: &str, value: &str) -> RunOutputDto {
    RunOutputDto { kind: kind.to_owned(), value: value.to_owned() }
}

fn asset_kind_as_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
        AssetKind::Audio => "audio",
    }
}

fn node_state_as_str(state: NodeExecutionState) -> &'static str {
    match state {
        NodeExecutionState::Idle => "idle",
        NodeExecutionState::Running => "running",
        NodeExecutionState::Done => "done",
        NodeExecutionState::Cached => "cached",
        NodeExecutionState::Error => "error",
    }
}
