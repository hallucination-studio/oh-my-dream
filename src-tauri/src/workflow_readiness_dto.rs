//! Canonical Workflow readiness command projection.

use engine::{
    node_capability::WorkflowDataType,
    workflow::{WorkflowReadinessIssue, WorkflowReadinessResult},
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::workflow_command_dto::WorkflowDto;

/// Current Workflow snapshot together with authoritative readiness.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowWithReadinessDto {
    /// Exact current Workflow.
    pub workflow: WorkflowDto,
    /// Closed readiness result.
    pub readiness: WorkflowReadinessDto,
}

/// Closed authoritative readiness result.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkflowReadinessDto {
    /// No current issue blocks execution.
    Ready,
    /// Sorted authoritative issues block execution.
    Blocked {
        /// Complete sorted issues.
        issues: Vec<Value>,
    },
}

pub(crate) fn readiness_dto(readiness: WorkflowReadinessResult) -> WorkflowReadinessDto {
    match readiness {
        WorkflowReadinessResult::Ready => WorkflowReadinessDto::Ready,
        WorkflowReadinessResult::Blocked { issues } => {
            WorkflowReadinessDto::Blocked { issues: issues.iter().map(readiness_issue).collect() }
        }
    }
}

fn readiness_issue(issue: &WorkflowReadinessIssue) -> Value {
    match issue {
        WorkflowReadinessIssue::WorkflowRequiredParameterMissing { node_id, parameter_key } => {
            issue_value(
                "required_parameter_missing",
                *node_id,
                json!({"parameter_key":parameter_key.as_str()}),
            )
        }
        WorkflowReadinessIssue::WorkflowRequiredInputMissing { node_id, input_key } => {
            issue_value("required_input_missing", *node_id, json!({"input_key":input_key.as_str()}))
        }
        WorkflowReadinessIssue::WorkflowReferenceMinimumNotMet {
            node_id,
            input_key,
            required_count,
            actual_count,
        } => issue_value(
            "reference_minimum_not_met",
            *node_id,
            json!({
                "input_key":input_key.as_str(),
                "required_count":required_count,
                "actual_count":actual_count,
            }),
        ),
        WorkflowReadinessIssue::WorkflowAssetUnavailable { node_id, input_key, asset_id } => {
            issue_value(
                "asset_unavailable",
                *node_id,
                json!({
                    "input_key":input_key.as_str(),
                    "asset_id":uuid::Uuid::from_bytes(asset_id.as_bytes()).hyphenated().to_string(),
                }),
            )
        }
        WorkflowReadinessIssue::WorkflowAssetKindMismatch {
            node_id,
            input_key,
            expected,
            actual,
        } => issue_value(
            "asset_kind_mismatch",
            *node_id,
            json!({
                "input_key":input_key.as_str(),
                "expected":data_type(*expected),
                "actual":data_type(*actual),
            }),
        ),
        WorkflowReadinessIssue::WorkflowCapabilityUnregistered { node_id, capability_ref } => {
            issue_value(
                "capability_unregistered",
                *node_id,
                json!({
                    "capability_id":capability_ref.id().as_str(),
                    "capability_version":format!(
                        "{}.{}",
                        capability_ref.version().major(),
                        capability_ref.version().minor()
                    ),
                }),
            )
        }
        WorkflowReadinessIssue::WorkflowGenerationProfileIncompatible {
            node_id,
            profile_ref,
            ..
        } => profile_issue("generation_profile_incompatible", *node_id, profile_ref),
        WorkflowReadinessIssue::WorkflowGenerationProfileUnavailable { node_id, profile_ref } => {
            profile_issue("generation_profile_unavailable", *node_id, profile_ref)
        }
        WorkflowReadinessIssue::WorkflowGenerationProfileAvailabilityIndeterminate {
            node_id,
            profile_ref,
        } => profile_issue("generation_profile_availability_indeterminate", *node_id, profile_ref),
        WorkflowReadinessIssue::WorkflowCapabilityExternalReadinessIssue { node_id, .. } => {
            issue_value("capability_external_readiness_issue", *node_id, json!({}))
        }
    }
}

fn profile_issue(
    kind: &'static str,
    node_id: engine::workflow_graph::WorkflowNodeId,
    profile: &engine::node_capability::NodeCapabilityGenerationProfileRefParameterValue,
) -> Value {
    issue_value(
        kind,
        node_id,
        json!({
            "profile_id":profile.profile_id(),
            "version":profile.version().to_string(),
        }),
    )
}

fn issue_value(
    kind: &'static str,
    node_id: engine::workflow_graph::WorkflowNodeId,
    detail: Value,
) -> Value {
    json!({
        "kind":kind,
        "node_id":node_id.as_uuid().hyphenated().to_string(),
        "detail":detail,
    })
}

const fn data_type(value: WorkflowDataType) -> &'static str {
    match value {
        WorkflowDataType::Text => "text",
        WorkflowDataType::Image => "image",
        WorkflowDataType::Video => "video",
        WorkflowDataType::Audio => "audio",
    }
}
