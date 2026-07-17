use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopErrorCode {
    ProjectInvalidRequest,
    ProjectNotFound,
    ProjectRevisionConflict,
    ProjectMutationConflict,
    NodeCapabilityInvalidRequest,
    WorkflowInvalidRequest,
    WorkflowNotFound,
    WorkflowRevisionConflict,
    WorkflowRunNotFound,
    WorkflowNotReady,
    WorkflowMutationConflict,
    AssetInvalidRequest,
    AssetNotFound,
    AssetNotVisible,
    AssetContentPending,
    AssetContentMissing,
    AssetInvalidMedia,
    ProviderUnavailable,
    AssistantNotFound,
    AssistantNotVisible,
    AssistantBusy,
    AssistantPendingApproval,
    AssistantApprovalExpired,
    AssistantApprovalMismatch,
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
    pub retry_after_epoch_ms: Option<String>,
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
            retry_after_epoch_ms: context
                .retry_after_epoch_ms
                .filter(|value| *value >= 0)
                .map(|value| value.to_string()),
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
        DesktopErrorCode::NodeCapabilityInvalidRequest => {
            ("node_capability.invalid_request", "The Node Capability request was invalid.")
        }
        DesktopErrorCode::WorkflowInvalidRequest => {
            ("workflow.invalid_request", "The Workflow request was invalid.")
        }
        DesktopErrorCode::WorkflowNotFound => ("workflow.not_found", "The Workflow was not found."),
        DesktopErrorCode::WorkflowRevisionConflict => {
            ("workflow.revision_conflict", "The Workflow changed. Reload and try again.")
        }
        DesktopErrorCode::WorkflowRunNotFound => {
            ("workflow_run.not_found", "The Workflow Run was not found.")
        }
        DesktopErrorCode::WorkflowNotReady => {
            ("workflow.not_ready", "The Workflow is not ready to run.")
        }
        DesktopErrorCode::WorkflowMutationConflict => {
            ("workflow.mutation_conflict", "The Workflow request conflicts with a prior request.")
        }
        DesktopErrorCode::AssetInvalidRequest => {
            ("asset.invalid_request", "The Asset request was invalid.")
        }
        DesktopErrorCode::AssetNotFound => ("asset.not_found", "The Asset was not found."),
        DesktopErrorCode::AssetNotVisible => {
            ("asset.not_visible", "The Asset is not visible in this Project.")
        }
        DesktopErrorCode::AssetContentPending => {
            ("asset.content_pending", "The Asset content is still being prepared.")
        }
        DesktopErrorCode::AssetContentMissing => {
            ("asset.content_missing", "The Asset content is unavailable.")
        }
        DesktopErrorCode::AssetInvalidMedia => {
            ("asset.invalid_media", "The selected file is not supported media.")
        }
        DesktopErrorCode::ProviderUnavailable => {
            ("provider.unavailable", "The selected generation provider is unavailable.")
        }
        DesktopErrorCode::AssistantNotFound => {
            ("assistant.not_found", "The Assistant change was not found.")
        }
        DesktopErrorCode::AssistantNotVisible => {
            ("assistant.not_visible", "The Assistant change is not visible in this Project.")
        }
        DesktopErrorCode::AssistantBusy => {
            ("assistant.busy", "An Assistant invocation is already active.")
        }
        DesktopErrorCode::AssistantPendingApproval => {
            ("assistant.pending_approval", "An Assistant change is awaiting a decision.")
        }
        DesktopErrorCode::AssistantApprovalExpired => {
            ("assistant.approval_expired", "The Assistant approval has expired.")
        }
        DesktopErrorCode::AssistantApprovalMismatch => {
            ("assistant.approval_mismatch", "The Assistant approval proof does not match.")
        }
        DesktopErrorCode::AssistantProtocolViolation => {
            ("assistant.protocol_violation", "The Assistant response was invalid.")
        }
        DesktopErrorCode::StorageUnavailable => {
            ("storage.unavailable", "Local storage is unavailable.")
        }
    }
}
