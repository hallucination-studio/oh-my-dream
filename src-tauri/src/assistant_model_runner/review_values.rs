use assistant::{domain::AssistantWorkflowChangeId, interfaces::AssistantApplicationError};
use serde_json::Value;

pub(super) fn reviewer_change_id(
    tool_id: &str,
    arguments: &Value,
) -> Result<Option<AssistantWorkflowChangeId>, AssistantApplicationError> {
    if tool_id != "assistant.workflow.get_change@1" {
        return Ok(None);
    }
    arguments
        .get("change_id")
        .and_then(Value::as_str)
        .ok_or(AssistantApplicationError::ReviewEvidenceInvalid)
        .and_then(parse_change_id)
        .map(Some)
}

pub(super) fn parse_change_id(
    value: &str,
) -> Result<AssistantWorkflowChangeId, AssistantApplicationError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)
        .and_then(|value| {
            AssistantWorkflowChangeId::from_uuid(value)
                .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)
        })
}
