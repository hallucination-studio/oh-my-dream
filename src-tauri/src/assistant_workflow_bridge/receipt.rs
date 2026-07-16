use assistant::interfaces::AssistantApplicationError;
use engine::workflow_graph::{WorkflowAggregate, WorkflowId, WorkflowRevision};

pub(super) fn apply_receipt_bytes(workflow: &WorkflowAggregate) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_bytes(&mut bytes, b"oh-my-dream/assistant-workflow-apply-receipt/v1");
    bytes.extend_from_slice(workflow.id.as_uuid().as_bytes());
    bytes.extend_from_slice(&workflow.revision.get().to_be_bytes());
    bytes.extend_from_slice(&workflow.canonical_fingerprint());
    bytes
}

pub(super) struct DecodedApplyReceipt {
    pub workflow_id: WorkflowId,
    pub workflow_revision: WorkflowRevision,
    pub workflow_fingerprint: [u8; 32],
}

pub(super) fn decode_apply_receipt(
    bytes: &[u8],
) -> Result<DecodedApplyReceipt, AssistantApplicationError> {
    const DOMAIN: &[u8] = b"oh-my-dream/assistant-workflow-apply-receipt/v1";
    let domain_length = bytes
        .get(..4)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(u32::from_be_bytes)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(AssistantApplicationError::ProtocolViolation)?;
    if domain_length != DOMAIN.len()
        || bytes.get(4..4 + domain_length) != Some(DOMAIN)
        || bytes.len() != 4 + DOMAIN.len() + 16 + 8 + 32
    {
        return Err(AssistantApplicationError::ProtocolViolation);
    }
    let identity_start = 4 + DOMAIN.len();
    let workflow_id = WorkflowId::from_uuid(uuid::Uuid::from_bytes(
        bytes[identity_start..identity_start + 16]
            .try_into()
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
    ))
    .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    let workflow_revision = WorkflowRevision::new(u64::from_be_bytes(
        bytes[identity_start + 16..identity_start + 24]
            .try_into()
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
    ))
    .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    let workflow_fingerprint = bytes[identity_start + 24..identity_start + 56]
        .try_into()
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    Ok(DecodedApplyReceipt { workflow_id, workflow_revision, workflow_fingerprint })
}

pub(super) fn run_boundary_bytes(run: &engine::workflow::WorkflowRunAggregate) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_bytes(&mut bytes, b"oh-my-dream/assistant-workflow-run/v1");
    bytes.extend_from_slice(run.run_id().as_uuid().as_bytes());
    bytes.extend_from_slice(run.project_id().as_uuid().as_bytes());
    bytes.extend_from_slice(run.workflow_id().as_uuid().as_bytes());
    bytes.extend_from_slice(&run.workflow_revision().get().to_be_bytes());
    bytes.push(run_state_tag(run.state()));
    bytes
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}

const fn run_state_tag(value: engine::workflow::WorkflowRunState) -> u8 {
    match value {
        engine::workflow::WorkflowRunState::Queued => 0,
        engine::workflow::WorkflowRunState::Running => 1,
        engine::workflow::WorkflowRunState::Succeeded => 2,
        engine::workflow::WorkflowRunState::Failed => 3,
        engine::workflow::WorkflowRunState::Cancelled => 4,
    }
}
