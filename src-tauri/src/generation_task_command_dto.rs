//! Sanitized Project-scoped Generation Task command projections.

use serde::{Deserialize, Serialize};
use tasks::generation_task::{
    GenerationProviderContract, GenerationTaskAggregate, GenerationTaskFailure,
    GenerationTaskFailureKind, GenerationTaskRequest, GenerationTaskRequestKind,
    GenerationTaskResult, GenerationTaskStatus, GenerationTaskSummaryView,
};

const MAX_PROMPT_PREVIEW_CHARS: usize = 512;

/// Project-scoped Task lookup request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationTaskGetRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Exact local Generation Task identity.
    pub generation_task_id: String,
}

/// Bounded Project-scoped Task list request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationTaskListRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Optional normalized lifecycle filter.
    pub status: Option<String>,
    /// Optional request-kind filter.
    pub request_kind: Option<String>,
    /// Opaque exclusive keyset cursor.
    pub cursor: Option<String>,
    /// Page size in `1..=100`.
    pub limit: u16,
}

/// Normalized Task lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationTaskStatusDto {
    /// Durable work has not started submission.
    Queued,
    /// Submission or accepted work is active.
    Running,
    /// Cancellation convergence is active.
    CancelRequested,
    /// A result was durably committed.
    Succeeded,
    /// A safe terminal failure was durably committed.
    Failed,
    /// Cancellation was durably committed.
    Cancelled,
}

/// Closed Generation Task request kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationTaskRequestKindDto {
    /// Text generation.
    Text,
    /// Image generation.
    Image,
    /// Video generation.
    Video,
    /// Voice generation.
    Voice,
}

/// Structured safe terminal failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationTaskFailureKindDto {
    /// Semantic request rejection.
    InvalidRequest,
    /// Provider authentication failure.
    Authentication,
    /// Provider authorization failure.
    PermissionDenied,
    /// Content policy rejection.
    ContentPolicy,
    /// Provider rate limiting.
    RateLimited,
    /// Provider is unavailable.
    ProviderUnavailable,
    /// Provider deadline elapsed.
    Timeout,
    /// Provider declared terminal rejection.
    ProviderRejected,
    /// Provider response was invalid.
    InvalidProviderResponse,
    /// Submission outcome could not be proven.
    AmbiguousSubmission,
    /// Exact input Asset was unavailable.
    InputAssetUnavailable,
    /// Generated output could not be finalized.
    OutputAssetImport,
    /// An internal invariant or adapter failed.
    Internal,
}

/// Safe failure details without provider response data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationTaskFailureDto {
    /// Closed failure category.
    pub kind: GenerationTaskFailureKindDto,
    /// Stable safe failure code.
    pub code: String,
    /// Bounded safe presentation message.
    pub message: String,
}

/// Tagged durable Task result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerationTaskResultDto {
    /// Inline generated text.
    Text { content: String },
    /// Durable generated media Asset identity.
    Asset { asset_id: String, media_kind: String },
}

/// Safe Task projection used by list and get.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationTaskSummaryDto {
    /// Local Task identity.
    pub id: String,
    /// Owning Project identity.
    pub project_id: String,
    /// Source Workflow identity.
    pub workflow_id: String,
    /// Source Workflow Run identity.
    pub workflow_run_id: String,
    /// Source Workflow node identity.
    pub workflow_node_id: String,
    /// Exact planned node execution identity.
    pub workflow_node_execution_id: String,
    /// Request-owned generation kind.
    pub request_kind: GenerationTaskRequestKindDto,
    /// Normalized current lifecycle status.
    pub status: GenerationTaskStatusDto,
    /// Known normalized progress.
    pub progress_percent: Option<u8>,
    /// Stable user-selectable Generation Profile reference.
    pub generation_profile_ref: String,
    /// Stable provider identity without route identity.
    pub provider_id: String,
    /// Current safe provider display name when still registered.
    pub provider_display_name: Option<String>,
    /// Bounded request prompt preview.
    pub prompt_preview: Option<String>,
    /// Durable output Asset identity when the result is media.
    pub preview_asset_id: Option<String>,
    /// Whether one durable result is present.
    pub has_result: bool,
    /// Safe terminal failure, when present.
    pub failure: Option<GenerationTaskFailureDto>,
    /// Creation time as non-negative UTC milliseconds.
    pub created_at_epoch_ms: String,
    /// Latest transition time as non-negative UTC milliseconds.
    pub updated_at_epoch_ms: String,
    /// Terminal completion time, when present.
    pub completed_at_epoch_ms: Option<String>,
}

/// Full safe Task projection returned by get.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationTaskDto {
    /// Shared safe Task fields.
    #[serde(flatten)]
    pub summary: GenerationTaskSummaryDto,
    /// Optional tagged durable result.
    pub result: Option<GenerationTaskResultDto>,
}

/// Stable bounded Task list page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationTaskListPageDto {
    /// Tasks in immutable descending creation order.
    pub tasks: Vec<GenerationTaskSummaryDto>,
    /// Opaque cursor only when another page exists.
    pub next_cursor: Option<String>,
}

pub(crate) fn task_dto(
    task: &GenerationTaskAggregate,
    provider_contracts: &[GenerationProviderContract],
) -> GenerationTaskDto {
    let result = task.result().map(result_dto);
    let summary = summary_fields(
        SummarySource {
            id: task.id().to_string(),
            origin: task.origin(),
            target: task.target(),
            request: task.request(),
            status: GenerationTaskStatus::from_state(task.state()),
            progress_percent: task.progress_percent(),
            result: result.as_ref(),
            failure: task.failure(),
            created_at_epoch_ms: task.created_at().as_utc_milliseconds().to_string(),
            updated_at_epoch_ms: task.updated_at().as_utc_milliseconds().to_string(),
            completed_at_epoch_ms: task
                .completed_at()
                .map(|value| value.as_utc_milliseconds().to_string()),
        },
        provider_contracts,
    );
    GenerationTaskDto { summary, result }
}

pub(crate) fn summary_dto(
    task: &GenerationTaskSummaryView,
    provider_contracts: &[GenerationProviderContract],
) -> GenerationTaskSummaryDto {
    let result = task.result.as_ref().map(result_dto);
    summary_fields(
        SummarySource {
            id: task.id.to_string(),
            origin: &task.origin,
            target: &task.target,
            request: &task.request,
            status: task.status,
            progress_percent: task.progress_percent,
            result: result.as_ref(),
            failure: task.failure.as_ref(),
            created_at_epoch_ms: task.created_at.as_utc_milliseconds().to_string(),
            updated_at_epoch_ms: task.updated_at.as_utc_milliseconds().to_string(),
            completed_at_epoch_ms: task
                .completed_at
                .map(|value| value.as_utc_milliseconds().to_string()),
        },
        provider_contracts,
    )
}

struct SummarySource<'a> {
    id: String,
    origin: &'a tasks::generation_task::GenerationTaskOrigin,
    target: &'a tasks::generation_task::GenerationTaskTarget,
    request: &'a GenerationTaskRequest,
    status: GenerationTaskStatus,
    progress_percent: Option<u8>,
    result: Option<&'a GenerationTaskResultDto>,
    failure: Option<&'a GenerationTaskFailure>,
    created_at_epoch_ms: String,
    updated_at_epoch_ms: String,
    completed_at_epoch_ms: Option<String>,
}

fn summary_fields(
    source: SummarySource<'_>,
    provider_contracts: &[GenerationProviderContract],
) -> GenerationTaskSummaryDto {
    GenerationTaskSummaryDto {
        id: source.id,
        project_id: source.origin.project_id().as_uuid().hyphenated().to_string(),
        workflow_id: source.origin.workflow_id().as_uuid().hyphenated().to_string(),
        workflow_run_id: source.origin.workflow_run_id().as_uuid().hyphenated().to_string(),
        workflow_node_id: source.origin.workflow_node_id().as_uuid().hyphenated().to_string(),
        workflow_node_execution_id: source
            .origin
            .workflow_node_execution_id()
            .as_uuid()
            .hyphenated()
            .to_string(),
        request_kind: request_kind_dto(source.request.kind()),
        status: status_dto(source.status),
        progress_percent: source.progress_percent,
        generation_profile_ref: source.target.generation_profile_ref().to_string(),
        provider_id: source.target.provider_id().as_str().to_owned(),
        provider_display_name: provider_contracts
            .iter()
            .find(|contract| contract.provider_id() == source.target.provider_id())
            .map(|contract| contract.display_name().as_str().to_owned()),
        prompt_preview: prompt_preview(source.request),
        preview_asset_id: source.result.and_then(result_asset_id),
        has_result: source.result.is_some(),
        failure: source.failure.map(failure_dto),
        created_at_epoch_ms: source.created_at_epoch_ms,
        updated_at_epoch_ms: source.updated_at_epoch_ms,
        completed_at_epoch_ms: source.completed_at_epoch_ms,
    }
}

fn result_dto(result: &GenerationTaskResult) -> GenerationTaskResultDto {
    match result {
        GenerationTaskResult::Text { content } => {
            GenerationTaskResultDto::Text { content: content.as_str().to_owned() }
        }
        GenerationTaskResult::Asset(value) => GenerationTaskResultDto::Asset {
            asset_id: value.asset_id().as_uuid().hyphenated().to_string(),
            media_kind: media_kind(value.media_kind()).to_owned(),
        },
    }
}

fn result_asset_id(result: &GenerationTaskResultDto) -> Option<String> {
    match result {
        GenerationTaskResultDto::Asset { asset_id, .. } => Some(asset_id.clone()),
        GenerationTaskResultDto::Text { .. } => None,
    }
}

fn failure_dto(failure: &GenerationTaskFailure) -> GenerationTaskFailureDto {
    GenerationTaskFailureDto {
        kind: failure_kind(failure.kind()),
        code: failure.code().to_owned(),
        message: failure.message().to_owned(),
    }
}

fn prompt_preview(request: &GenerationTaskRequest) -> Option<String> {
    let prompt = match request {
        GenerationTaskRequest::Text(value) => Some(value.prompt().as_str()),
        GenerationTaskRequest::Image(value) => Some(value.prompt().as_str()),
        GenerationTaskRequest::Voice(value) => Some(value.text().as_str()),
        GenerationTaskRequest::Video(value) => value.prompt().map(|prompt| prompt.as_str()),
    }?;
    Some(prompt.chars().take(MAX_PROMPT_PREVIEW_CHARS).collect())
}

const fn status_dto(status: GenerationTaskStatus) -> GenerationTaskStatusDto {
    match status {
        GenerationTaskStatus::Queued => GenerationTaskStatusDto::Queued,
        GenerationTaskStatus::Running => GenerationTaskStatusDto::Running,
        GenerationTaskStatus::CancelRequested => GenerationTaskStatusDto::CancelRequested,
        GenerationTaskStatus::Succeeded => GenerationTaskStatusDto::Succeeded,
        GenerationTaskStatus::Failed => GenerationTaskStatusDto::Failed,
        GenerationTaskStatus::Cancelled => GenerationTaskStatusDto::Cancelled,
    }
}

const fn request_kind_dto(kind: GenerationTaskRequestKind) -> GenerationTaskRequestKindDto {
    match kind {
        GenerationTaskRequestKind::Text => GenerationTaskRequestKindDto::Text,
        GenerationTaskRequestKind::Image => GenerationTaskRequestKindDto::Image,
        GenerationTaskRequestKind::Video => GenerationTaskRequestKindDto::Video,
        GenerationTaskRequestKind::Voice => GenerationTaskRequestKindDto::Voice,
    }
}

const fn failure_kind(kind: GenerationTaskFailureKind) -> GenerationTaskFailureKindDto {
    match kind {
        GenerationTaskFailureKind::InvalidRequest => GenerationTaskFailureKindDto::InvalidRequest,
        GenerationTaskFailureKind::Authentication => GenerationTaskFailureKindDto::Authentication,
        GenerationTaskFailureKind::PermissionDenied => {
            GenerationTaskFailureKindDto::PermissionDenied
        }
        GenerationTaskFailureKind::ContentPolicy => GenerationTaskFailureKindDto::ContentPolicy,
        GenerationTaskFailureKind::RateLimited => GenerationTaskFailureKindDto::RateLimited,
        GenerationTaskFailureKind::ProviderUnavailable => {
            GenerationTaskFailureKindDto::ProviderUnavailable
        }
        GenerationTaskFailureKind::Timeout => GenerationTaskFailureKindDto::Timeout,
        GenerationTaskFailureKind::ProviderRejected => {
            GenerationTaskFailureKindDto::ProviderRejected
        }
        GenerationTaskFailureKind::InvalidProviderResponse => {
            GenerationTaskFailureKindDto::InvalidProviderResponse
        }
        GenerationTaskFailureKind::AmbiguousSubmission => {
            GenerationTaskFailureKindDto::AmbiguousSubmission
        }
        GenerationTaskFailureKind::InputAssetUnavailable => {
            GenerationTaskFailureKindDto::InputAssetUnavailable
        }
        GenerationTaskFailureKind::OutputAssetImport => {
            GenerationTaskFailureKindDto::OutputAssetImport
        }
        GenerationTaskFailureKind::Internal => GenerationTaskFailureKindDto::Internal,
    }
}

fn media_kind(value: assets::asset::domain::AssetMediaKind) -> &'static str {
    match value {
        assets::asset::domain::AssetMediaKind::Image => "image",
        assets::asset::domain::AssetMediaKind::Video => "video",
        assets::asset::domain::AssetMediaKind::Audio => "audio",
    }
}
