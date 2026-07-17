//! Canonical four-shell Workflow node presentation projection.

use engine::{
    node_capability::{WorkflowTextPart, WorkflowTextValue},
    workflow::{
        WorkflowNodeExecutionState, WorkflowNodePresentationShell, WorkflowNodePresentationView,
        WorkflowReadinessResult,
    },
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::workflow_readiness_dto::{WorkflowReadinessDto, readiness_dto};

/// Current node facts and exactly one primary-output presentation shell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowNodePresentationDto {
    /// Current node identity.
    pub node_id: String,
    /// Current Workflow revision as decimal text.
    pub current_revision: String,
    /// Exact capability family ID.
    pub capability_id: String,
    /// Exact capability `major.minor` version.
    pub capability_version: String,
    /// Current node-scoped readiness.
    pub readiness: WorkflowReadinessDto,
    /// Latest relevant execution summary, or `null`.
    pub latest_execution: Option<WorkflowNodeExecutionSummaryDto>,
    /// Closed Text, Image, Video, or Audio shell.
    pub presentation: Value,
}

/// Safe latest-execution facts used by React presentation state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowNodeExecutionSummaryDto {
    /// Producing Run identity.
    pub workflow_run_id: String,
    /// Producing node execution identity.
    pub node_execution_id: String,
    /// Durable execution state.
    pub state: String,
    /// Optional basis-point progress.
    pub progress_basis_points: Option<u16>,
    /// Frozen producing Workflow revision.
    pub producing_revision: String,
    /// Whether the current node or any ancestor has changed.
    pub is_stale: bool,
    /// Structured capability failure when failed.
    pub failure: Option<Value>,
    /// Structured upstream block reason when blocked.
    pub block_reason: Option<Value>,
}

pub(crate) fn node_presentation_dto(
    view: WorkflowNodePresentationView,
) -> WorkflowNodePresentationDto {
    let readiness = if view.readiness_issues.is_empty() {
        WorkflowReadinessResult::Ready
    } else {
        WorkflowReadinessResult::from_issues(view.readiness_issues)
    };
    WorkflowNodePresentationDto {
        node_id: view.node_id.as_uuid().hyphenated().to_string(),
        current_revision: view.current_revision.get().to_string(),
        capability_id: view.capability_ref.id().as_str().to_owned(),
        capability_version: format!(
            "{}.{}",
            view.capability_ref.version().major(),
            view.capability_ref.version().minor()
        ),
        readiness: readiness_dto(readiness),
        latest_execution: view.latest_execution.map(|value| WorkflowNodeExecutionSummaryDto {
            workflow_run_id: value.workflow_run_id.as_uuid().hyphenated().to_string(),
            node_execution_id: value.node_execution_id.as_uuid().hyphenated().to_string(),
            state: execution_state(value.state).to_owned(),
            progress_basis_points: value.progress_basis_points,
            producing_revision: value.producing_revision.get().to_string(),
            is_stale: value.is_stale,
            failure: value.failure.as_ref().map(|failure| {
                crate::assistant_workflow_bridge::run_projection::execution_failure(
                    &failure.capability_error,
                )
            }),
            block_reason: value
                .block_reason
                .as_ref()
                .map(crate::assistant_workflow_bridge::run_projection::block_reason),
        }),
        presentation: shell_dto(view.shell),
    }
}

fn shell_dto(shell: WorkflowNodePresentationShell) -> Value {
    match shell {
        WorkflowNodePresentationShell::Text(value) => json!({
            "kind":"text",
            "value":value.value.as_ref().map(text_value),
        }),
        WorkflowNodePresentationShell::Image(value) => media_shell(
            "image",
            value
                .value
                .map(|value| (value.asset_id().as_bytes(), value.content_fingerprint().as_bytes())),
            value.preview.map(|value| value.as_str().to_owned()),
        ),
        WorkflowNodePresentationShell::Video(value) => media_shell(
            "video",
            value
                .value
                .map(|value| (value.asset_id().as_bytes(), value.content_fingerprint().as_bytes())),
            value.preview.map(|value| value.as_str().to_owned()),
        ),
        WorkflowNodePresentationShell::Audio(value) => media_shell(
            "audio",
            value
                .value
                .map(|value| (value.asset_id().as_bytes(), value.content_fingerprint().as_bytes())),
            value.preview.map(|value| value.as_str().to_owned()),
        ),
    }
}

fn text_value(value: &WorkflowTextValue) -> Value {
    Value::Array(
        value
            .parts()
            .iter()
            .map(|part| match part {
                WorkflowTextPart::Literal(value) => json!({"kind":"literal","value":value}),
                WorkflowTextPart::InputItemReference(id) => json!({
                    "kind":"input_item_reference",
                    "input_item_id":id.as_uuid().hyphenated().to_string(),
                }),
            })
            .collect(),
    )
}

fn media_shell(
    kind: &'static str,
    value: Option<([u8; 16], [u8; 32])>,
    preview_uri: Option<String>,
) -> Value {
    json!({
        "kind":kind,
        "value":value.map(|(asset_id, fingerprint)| json!({
            "asset_id":uuid::Uuid::from_bytes(asset_id).hyphenated().to_string(),
            "content_fingerprint_hex":hex(&fingerprint),
        })),
        "preview_uri":preview_uri,
    })
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn execution_state(value: WorkflowNodeExecutionState) -> &'static str {
    match value {
        WorkflowNodeExecutionState::Pending => "pending",
        WorkflowNodeExecutionState::Running => "running",
        WorkflowNodeExecutionState::Succeeded => "succeeded",
        WorkflowNodeExecutionState::Failed => "failed",
        WorkflowNodeExecutionState::Cancelled => "cancelled",
        WorkflowNodeExecutionState::Blocked => "blocked",
    }
}

#[cfg(test)]
mod tests {
    use engine::{
        node_capability::{
            NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
            WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
            WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
            WorkflowTextPart, WorkflowTextValue,
        },
        workflow::{
            WorkflowAudioNodePresentation, WorkflowImageNodePresentation, WorkflowMediaPreview,
            WorkflowNodePresentationShell, WorkflowNodePresentationView,
            WorkflowTextNodePresentation, WorkflowVideoNodePresentation,
        },
        workflow_graph::{WorkflowNodeId, WorkflowRevision},
    };
    use uuid::Uuid;

    use super::*;

    #[test]
    fn projection_keeps_the_closed_text_and_media_shell_tags() {
        let text = node_presentation_dto(view(WorkflowNodePresentationShell::Text(
            WorkflowTextNodePresentation {
                value: Some(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal("hello".to_owned())])
                        .unwrap(),
                ),
            },
        )));
        let image_ref = WorkflowManagedImageRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(*uuid(2).as_bytes()).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes([3; 32]),
        );
        let image = node_presentation_dto(view(WorkflowNodePresentationShell::Image(
            WorkflowImageNodePresentation {
                value: Some(image_ref),
                preview: Some(WorkflowMediaPreview::try_new("desktop-asset://v1/token").unwrap()),
            },
        )));
        let video = node_presentation_dto(view(WorkflowNodePresentationShell::Video(
            WorkflowVideoNodePresentation {
                value: Some(WorkflowManagedVideoRef::new(
                    WorkflowManagedAssetIdBoundaryValue::from_bytes(*uuid(4).as_bytes()).unwrap(),
                    WorkflowManagedContentFingerprint::from_bytes([5; 32]),
                )),
                preview: None,
            },
        )));
        let audio = node_presentation_dto(view(WorkflowNodePresentationShell::Audio(
            WorkflowAudioNodePresentation {
                value: Some(WorkflowManagedAudioRef::new(
                    WorkflowManagedAssetIdBoundaryValue::from_bytes(*uuid(6).as_bytes()).unwrap(),
                    WorkflowManagedContentFingerprint::from_bytes([7; 32]),
                )),
                preview: None,
            },
        )));
        assert_eq!(text.presentation["kind"], "text");
        assert_eq!(image.presentation["kind"], "image");
        assert_eq!(video.presentation["kind"], "video");
        assert_eq!(audio.presentation["kind"], "audio");
        assert_eq!(image.presentation["preview_uri"], "desktop-asset://v1/token");
    }

    fn view(shell: WorkflowNodePresentationShell) -> WorkflowNodePresentationView {
        WorkflowNodePresentationView {
            node_id: WorkflowNodeId::from_uuid(uuid(1)).unwrap(),
            current_revision: WorkflowRevision::new(1).unwrap(),
            capability_ref: NodeCapabilityContractRef::new(
                NodeCapabilityContractId::new("text.provide_literal").unwrap(),
                NodeCapabilityContractVersion::new(1, 0).unwrap(),
            ),
            readiness_issues: Vec::new(),
            latest_execution: None,
            shell,
        }
    }

    fn uuid(seed: u128) -> Uuid {
        Uuid::from_u128(0x123e_4567_e89b_42d3_a456_0000_0000_0000 | seed)
    }
}
