use engine::{
    node_capability::{
        NodeCapabilityExecutionFailure, NodeCapabilityExecutionTarget, NodeCapabilityMediaFailure,
        NodeCapabilityProviderFailureCategory, NodeCapabilityReadinessTarget, WorkflowRuntimeValue,
        WorkflowTextPart,
    },
    workflow::{
        WorkflowNodeExecutionBlockReason, WorkflowRunAggregate, WorkflowRunEvent,
        WorkflowRunEventPayload, WorkflowRunFailure, WorkflowRunState,
    },
};
use serde_json::{Value, json};

pub fn run_with_events_boundary(
    run: &WorkflowRunAggregate,
    events: &[WorkflowRunEvent],
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&json!({
        "version": 1,
        "run": {
            "run_id": run.run_id().as_uuid().to_string(),
            "project_id": run.project_id().as_uuid().to_string(),
            "workflow_id": run.workflow_id().as_uuid().to_string(),
            "workflow_revision": run.workflow_revision().get(),
            "state": run_state(run.state()),
            "failure": run.failure().map(run_failure),
            "created_at_epoch_ms": run.created_at().as_utc_milliseconds(),
            "updated_at_epoch_ms": run.updated_at().as_utc_milliseconds(),
        },
        "events": events.iter().map(workflow_run_event_value).collect::<Vec<_>>(),
    }))
}

pub(crate) fn workflow_run_event_value(value: &WorkflowRunEvent) -> Value {
    json!({
        "sequence": value.sequence().get(),
        "occurred_at_epoch_ms": value.occurred_at().as_utc_milliseconds(),
        "payload": event_payload(value.payload()),
    })
}

fn event_payload(value: &WorkflowRunEventPayload) -> Value {
    match value {
        WorkflowRunEventPayload::WorkflowRunQueuedEvent => json!({"type": "run_queued"}),
        WorkflowRunEventPayload::WorkflowRunStartedEvent => json!({"type": "run_started"}),
        WorkflowRunEventPayload::WorkflowNodeStartedEvent { node_execution_id } => {
            execution_event("node_started", *node_execution_id)
        }
        WorkflowRunEventPayload::WorkflowNodeProgressedEvent {
            node_execution_id,
            progress_basis_points,
        } => json!({
            "type": "node_progressed",
            "node_execution_id": node_execution_id.as_uuid().to_string(),
            "progress_basis_points": progress_basis_points,
        }),
        WorkflowRunEventPayload::WorkflowNodeWaitingForExternalCompletionEvent {
            node_execution_id,
        } => execution_event("node_waiting_for_external_completion", *node_execution_id),
        WorkflowRunEventPayload::WorkflowNodeSucceededEvent { node_execution_id, outputs } => {
            json!({
                "type": "node_succeeded",
                "node_execution_id": node_execution_id.as_uuid().to_string(),
                "outputs": outputs.iter().map(|(key, value)| json!({
                    "key": key.as_str(),
                    "value": runtime_value(value),
                })).collect::<Vec<_>>(),
            })
        }
        WorkflowRunEventPayload::WorkflowNodeFailedEvent { node_execution_id, failure } => json!({
            "type": "node_failed",
            "node_execution_id": node_execution_id.as_uuid().to_string(),
            "failure": node_execution_failure(failure),
        }),
        WorkflowRunEventPayload::WorkflowNodeBlockedEvent { node_execution_id, reason } => json!({
            "type": "node_blocked",
            "node_execution_id": node_execution_id.as_uuid().to_string(),
            "reason": block_reason(reason),
        }),
        WorkflowRunEventPayload::WorkflowNodeCancelledEvent { node_execution_id } => {
            execution_event("node_cancelled", *node_execution_id)
        }
        WorkflowRunEventPayload::WorkflowRunSucceededEvent => json!({"type": "run_succeeded"}),
        WorkflowRunEventPayload::WorkflowRunFailedEvent { failure } => {
            json!({"type": "run_failed", "failure": run_failure(failure)})
        }
        WorkflowRunEventPayload::WorkflowRunCancelledEvent => json!({"type": "run_cancelled"}),
    }
}

fn execution_event(
    kind: &'static str,
    id: engine::node_capability::WorkflowNodeExecutionId,
) -> Value {
    json!({"type": kind, "node_execution_id": id.as_uuid().to_string()})
}

fn run_failure(value: &WorkflowRunFailure) -> Value {
    match value {
        WorkflowRunFailure::NodeExecutionFailed { sorted_failed_node_ids } => json!({
            "type": "node_execution_failed",
            "node_ids": sorted_failed_node_ids
                .iter()
                .map(|id| id.as_uuid().to_string())
                .collect::<Vec<_>>(),
        }),
        WorkflowRunFailure::InterruptedByRestart => json!({"type": "interrupted_by_restart"}),
    }
}

pub(crate) fn block_reason(value: &WorkflowNodeExecutionBlockReason) -> Value {
    match value {
        WorkflowNodeExecutionBlockReason::UpstreamNodeFailed { sorted_upstream_node_ids } => {
            json!({
                "type": "upstream_node_failed",
                "node_ids": sorted_upstream_node_ids
                    .iter()
                    .map(|id| id.as_uuid().to_string())
                    .collect::<Vec<_>>(),
            })
        }
    }
}

pub(crate) fn execution_failure(
    value: &engine::node_capability::NodeCapabilityExecutionError,
) -> Value {
    json!({
        "contract_ref": {
            "id": value.contract_ref().id().as_str(),
            "major": value.contract_ref().version().major(),
            "minor": value.contract_ref().version().minor(),
        },
        "node_execution_id": value.node_execution_id().as_uuid().to_string(),
        "stage": execution_stage(value.stage()),
        "failure": failure_source(value.failure()),
        "target": execution_target(value.target()),
    })
}

pub(crate) fn node_execution_failure(
    value: &engine::workflow::WorkflowNodeExecutionFailure,
) -> Value {
    match value {
        engine::workflow::WorkflowNodeExecutionFailure::Capability(error) => {
            execution_failure(error)
        }
        engine::workflow::WorkflowNodeExecutionFailure::GenerationTask(failure) => json!({
            "type": "generation_task",
            "category": generation_task_failure(*failure),
        }),
    }
}

fn failure_source(value: &NodeCapabilityExecutionFailure) -> Value {
    match value {
        NodeCapabilityExecutionFailure::InvalidCapabilityInvocation => {
            json!({"type": "invalid_capability_invocation"})
        }
        NodeCapabilityExecutionFailure::InvalidCapabilityResult => {
            json!({"type": "invalid_capability_result"})
        }
        NodeCapabilityExecutionFailure::Readiness(issue) => json!({
            "type": "readiness",
            "category": readiness_category(issue.category()),
            "target": readiness_target(issue.target()),
            "media_kind_mismatch": issue.media_kind_mismatch().map(|(expected, observed)| json!({
                "expected": data_type(expected),
                "observed": data_type(observed),
            })),
        }),
        NodeCapabilityExecutionFailure::Provider(failure) => json!({
            "type": "provider",
            "category": provider_category(failure.category()),
            "retryable": failure.is_retryable(),
        }),
        NodeCapabilityExecutionFailure::Media(failure) => {
            json!({"type": "media", "detail": media_failure(failure)})
        }
        NodeCapabilityExecutionFailure::GenerationTaskStart(failure) => json!({
            "type": "generation_task_start",
            "category": generation_task_start_failure(*failure),
        }),
        NodeCapabilityExecutionFailure::Cancelled => json!({"type": "cancelled"}),
        NodeCapabilityExecutionFailure::DeadlineExceeded => json!({"type": "deadline_exceeded"}),
    }
}

fn readiness_target(value: &NodeCapabilityReadinessTarget) -> Value {
    match value {
        NodeCapabilityReadinessTarget::Capability => json!({"type": "capability"}),
        NodeCapabilityReadinessTarget::ManagedAsset { parameter_key, asset_id } => json!({
            "type": "managed_asset",
            "parameter_key": parameter_key.as_str(),
            "asset_id": uuid::Uuid::from_bytes(asset_id.as_bytes()).to_string(),
        }),
        NodeCapabilityReadinessTarget::GenerationProfile {
            parameter_key,
            generation_profile_ref,
        } => json!({
            "type": "generation_profile",
            "parameter_key": parameter_key.as_str(),
            "profile_id": generation_profile_ref.profile_id(),
            "profile_version": generation_profile_ref.version(),
        }),
    }
}

fn execution_target(value: &NodeCapabilityExecutionTarget) -> Value {
    match value {
        NodeCapabilityExecutionTarget::Capability => json!({"type": "capability"}),
        NodeCapabilityExecutionTarget::Parameter(key) => {
            json!({"type": "parameter", "key": key.as_str()})
        }
        NodeCapabilityExecutionTarget::Input(key) => json!({"type": "input", "key": key.as_str()}),
        NodeCapabilityExecutionTarget::Output(key) => {
            json!({"type": "output", "key": key.as_str()})
        }
    }
}

fn runtime_value(value: &WorkflowRuntimeValue) -> Value {
    match value {
        WorkflowRuntimeValue::Text(text) => json!({
            "type": "text",
            "parts": text.parts().iter().map(|part| match part {
                WorkflowTextPart::Literal(value) => json!({"type": "literal", "value": value}),
                WorkflowTextPart::InputItemReference(id) => {
                    json!({"type": "input_item_reference", "id": id.as_uuid().to_string()})
                }
            }).collect::<Vec<_>>(),
        }),
        WorkflowRuntimeValue::Image(reference) => managed_media(
            "image",
            reference.asset_id().as_bytes(),
            reference.content_fingerprint().as_bytes(),
        ),
        WorkflowRuntimeValue::Video(reference) => managed_media(
            "video",
            reference.asset_id().as_bytes(),
            reference.content_fingerprint().as_bytes(),
        ),
        WorkflowRuntimeValue::Audio(reference) => managed_media(
            "audio",
            reference.asset_id().as_bytes(),
            reference.content_fingerprint().as_bytes(),
        ),
    }
}

fn managed_media(kind: &'static str, asset_id: [u8; 16], fingerprint: [u8; 32]) -> Value {
    json!({
        "type": kind,
        "asset_id": uuid::Uuid::from_bytes(asset_id).to_string(),
        "content_fingerprint_hex": hex(&fingerprint),
    })
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn run_state(value: WorkflowRunState) -> &'static str {
    match value {
        WorkflowRunState::Queued => "queued",
        WorkflowRunState::Running => "running",
        WorkflowRunState::Succeeded => "succeeded",
        WorkflowRunState::Failed => "failed",
        WorkflowRunState::Cancelled => "cancelled",
    }
}

macro_rules! enum_tag {
    ($name:ident, $type:ty, {$($variant:path => $tag:literal),+ $(,)?}) => {
        const fn $name(value: $type) -> &'static str {
            match value {$($variant => $tag),+}
        }
    };
}

enum_tag!(execution_stage, engine::node_capability::NodeCapabilityExecutionStage, {
    engine::node_capability::NodeCapabilityExecutionStage::ResolveInputs => "resolve_inputs",
    engine::node_capability::NodeCapabilityExecutionStage::StartGenerationTask => "start_generation_task",
    engine::node_capability::NodeCapabilityExecutionStage::CallProvider => "call_provider",
    engine::node_capability::NodeCapabilityExecutionStage::ValidateProviderResult => "validate_provider_result",
    engine::node_capability::NodeCapabilityExecutionStage::WriteManagedMedia => "write_managed_media",
    engine::node_capability::NodeCapabilityExecutionStage::AssembleOutputs => "assemble_outputs",
});

enum_tag!(generation_task_start_failure, engine::node_capability::NodeCapabilityGenerationTaskStartFailure, {
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::InvalidRequest => "invalid_request",
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::Conflict => "conflict",
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::Unavailable => "unavailable",
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::Cancelled => "cancelled",
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded => "deadline_exceeded",
    engine::node_capability::NodeCapabilityGenerationTaskStartFailure::Persistence => "persistence",
});

enum_tag!(generation_task_failure, engine::workflow::WorkflowGenerationTaskFailure, {
    engine::workflow::WorkflowGenerationTaskFailure::InvalidRequest => "invalid_request",
    engine::workflow::WorkflowGenerationTaskFailure::Authentication => "authentication",
    engine::workflow::WorkflowGenerationTaskFailure::PermissionDenied => "permission_denied",
    engine::workflow::WorkflowGenerationTaskFailure::ContentPolicy => "content_policy",
    engine::workflow::WorkflowGenerationTaskFailure::RateLimited => "rate_limited",
    engine::workflow::WorkflowGenerationTaskFailure::ProviderUnavailable => "provider_unavailable",
    engine::workflow::WorkflowGenerationTaskFailure::Timeout => "timeout",
    engine::workflow::WorkflowGenerationTaskFailure::ProviderRejected => "provider_rejected",
    engine::workflow::WorkflowGenerationTaskFailure::InvalidProviderResponse => "invalid_provider_response",
    engine::workflow::WorkflowGenerationTaskFailure::AmbiguousSubmission => "ambiguous_submission",
    engine::workflow::WorkflowGenerationTaskFailure::InputAssetUnavailable => "input_asset_unavailable",
    engine::workflow::WorkflowGenerationTaskFailure::OutputAssetImport => "output_asset_import",
    engine::workflow::WorkflowGenerationTaskFailure::Internal => "internal",
    engine::workflow::WorkflowGenerationTaskFailure::GenerationTaskCancelled => "generation_task_cancelled",
});

enum_tag!(provider_category, NodeCapabilityProviderFailureCategory, {
    NodeCapabilityProviderFailureCategory::InvalidSemanticRequest => "invalid_semantic_request",
    NodeCapabilityProviderFailureCategory::AuthenticationFailed => "authentication_failed",
    NodeCapabilityProviderFailureCategory::PermissionDenied => "permission_denied",
    NodeCapabilityProviderFailureCategory::ContentPolicyRejected => "content_policy_rejected",
    NodeCapabilityProviderFailureCategory::RateLimited => "rate_limited",
    NodeCapabilityProviderFailureCategory::ProviderUnavailable => "provider_unavailable",
    NodeCapabilityProviderFailureCategory::DeadlineExceeded => "deadline_exceeded",
    NodeCapabilityProviderFailureCategory::ProviderRejected => "provider_rejected",
    NodeCapabilityProviderFailureCategory::InvalidResponse => "invalid_response",
    NodeCapabilityProviderFailureCategory::DownloadRejected => "download_rejected",
    NodeCapabilityProviderFailureCategory::AmbiguousSubmission => "ambiguous_submission",
});

fn media_failure(value: &NodeCapabilityMediaFailure) -> Value {
    match value {
        NodeCapabilityMediaFailure::Unavailable => json!({"category": "unavailable"}),
        NodeCapabilityMediaFailure::KindMismatch { expected, observed } => json!({
            "category": "kind_mismatch",
            "expected": data_type(*expected),
            "observed": data_type(*observed),
        }),
        NodeCapabilityMediaFailure::InvalidMedia => json!({"category": "invalid_media"}),
        NodeCapabilityMediaFailure::SizeLimitExceeded => {
            json!({"category": "size_limit_exceeded"})
        }
        NodeCapabilityMediaFailure::DigestMismatch => json!({"category": "digest_mismatch"}),
        NodeCapabilityMediaFailure::OutputConflict => json!({"category": "output_conflict"}),
        NodeCapabilityMediaFailure::StorageFailed => json!({"category": "storage_failed"}),
        NodeCapabilityMediaFailure::InspectionFailed => json!({"category": "inspection_failed"}),
        NodeCapabilityMediaFailure::FinalizationFailed => {
            json!({"category": "finalization_failed"})
        }
    }
}

enum_tag!(readiness_category, engine::node_capability::NodeCapabilityReadinessCategory, {
    engine::node_capability::NodeCapabilityReadinessCategory::InvalidCapabilityInvocation => "invalid_capability_invocation",
    engine::node_capability::NodeCapabilityReadinessCategory::ManagedAssetUnavailable => "managed_asset_unavailable",
    engine::node_capability::NodeCapabilityReadinessCategory::ManagedAssetKindMismatch => "managed_asset_kind_mismatch",
    engine::node_capability::NodeCapabilityReadinessCategory::ManagedAssetReadinessIndeterminate => "managed_asset_readiness_indeterminate",
    engine::node_capability::NodeCapabilityReadinessCategory::GenerationProfileIncompatible => "generation_profile_incompatible",
    engine::node_capability::NodeCapabilityReadinessCategory::GenerationProfileUnavailable => "generation_profile_unavailable",
    engine::node_capability::NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate => "generation_profile_availability_indeterminate",
});

enum_tag!(data_type, engine::node_capability::WorkflowDataType, {
    engine::node_capability::WorkflowDataType::Text => "text",
    engine::node_capability::WorkflowDataType::Image => "image",
    engine::node_capability::WorkflowDataType::Video => "video",
    engine::node_capability::WorkflowDataType::Audio => "audio",
});
