use super::*;

pub(super) struct CandidateBase {
    pub workflow: Workflow,
    pub patches: Vec<WorkflowPatch>,
    pub aliases: Vec<(String, String)>,
}

pub(super) fn new_candidate(
    input: PrepareCandidateInput,
    patches: Vec<WorkflowPatch>,
    aliases: Vec<(String, String)>,
    result: engine::WorkflowPatchResult,
) -> Result<WorkflowCandidate, ReviewedChangeError> {
    let digest = fingerprint(&patches)?;
    let workflow_fingerprint = fingerprint(&result.workflow)?;
    let sequence = NEXT_CANDIDATE_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ReviewedChangeError::Clock(error.to_string()))?
        .as_nanos();
    Ok(WorkflowCandidate {
        id: format!("candidate-{timestamp:032x}-{sequence:016x}"),
        project_id: input.project_id,
        session_id: input.session_id,
        user_intent: input.user_intent,
        base_revision: input.expected_revision,
        patches,
        digest,
        workflow_fingerprint,
        workflow: result.workflow,
        aliases,
        readiness_blockers: result.readiness_blockers,
        expires_at: now_seconds()?.saturating_add(CANDIDATE_TTL_SECONDS),
    })
}

pub(super) fn fingerprint(value: &impl Serialize) -> Result<String, ReviewedChangeError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| ReviewedChangeError::Storage(error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

pub(super) fn now_seconds() -> Result<u64, ReviewedChangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| ReviewedChangeError::Clock(error.to_string()))
}
