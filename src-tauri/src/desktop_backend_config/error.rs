use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopErrorCode {
    ProjectInvalidRequest,
    ProjectNotFound,
    ProjectRevisionConflict,
    ProjectMutationConflict,
    WorkflowRevisionConflict,
    ProviderUnavailable,
    AssistantProtocolViolation,
    StorageUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopErrorContext {
    pub code: DesktopErrorCode,
    pub retryable: bool,
    pub retry_after_epoch_ms: Option<i64>,
    pub target: Option<DesktopErrorTarget>,
    pub correlation_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopErrorTarget {
    Project { project_id: String },
    Workflow { workflow_id: String },
    Run { workflow_run_id: String },
    Node { workflow_node_id: String },
    Asset { asset_id: String },
    AssistantChange { assistant_workflow_change_id: String },
    GenerationProfile { generation_profile_ref: String },
    Parameter { parameter_id: String },
    Input { input_id: String },
    Output { output_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DesktopErrorDto {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
    pub retry_after_epoch_ms: Option<i64>,
    pub target: Option<DesktopErrorTarget>,
    pub correlation_id: Option<String>,
}

impl DesktopErrorDto {
    #[must_use]
    pub fn from_context(context: DesktopErrorContext) -> Self {
        let (code, message) = safe_error(context.code);
        Self {
            code,
            message,
            retryable: context.retryable,
            retry_after_epoch_ms: context.retry_after_epoch_ms.filter(|value| *value >= 0),
            target: context.target,
            correlation_id: context.correlation_id.map(|value| value.to_string()),
        }
    }

    #[must_use]
    pub fn internal(correlation_id: Uuid) -> Self {
        Self {
            code: "desktop.internal",
            message: "An internal error occurred.",
            retryable: false,
            retry_after_epoch_ms: None,
            target: None,
            correlation_id: Some(correlation_id.to_string()),
        }
    }
}

fn safe_error(code: DesktopErrorCode) -> (&'static str, &'static str) {
    match code {
        DesktopErrorCode::ProjectInvalidRequest => {
            ("project.invalid_request", "The Project request was invalid.")
        }
        DesktopErrorCode::ProjectNotFound => ("project.not_found", "The Project was not found."),
        DesktopErrorCode::ProjectRevisionConflict => {
            ("project.revision_conflict", "The Project changed. Reload and try again.")
        }
        DesktopErrorCode::ProjectMutationConflict => {
            ("project.mutation_conflict", "The Project request conflicts with a prior request.")
        }
        DesktopErrorCode::WorkflowRevisionConflict => {
            ("workflow.revision_conflict", "The Workflow changed. Reload and try again.")
        }
        DesktopErrorCode::ProviderUnavailable => {
            ("provider.unavailable", "The selected generation provider is unavailable.")
        }
        DesktopErrorCode::AssistantProtocolViolation => {
            ("assistant.protocol_violation", "The Assistant response was invalid.")
        }
        DesktopErrorCode::StorageUnavailable => {
            ("storage.unavailable", "Local storage is unavailable.")
        }
    }
}
