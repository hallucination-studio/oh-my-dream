use super::*;

impl WorkflowPatchService {
    /// Replays an exact patch sequence and commits the final Workflow once.
    pub fn apply_sequence(
        &self,
        context: &RequestContext,
        expected_revision: Option<u64>,
        patches: &[WorkflowPatch],
    ) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
        self.ensure_project(context.project_id(), None)?;
        let current = self
            .authority
            .load_head(context.project_id())
            .map_err(|error| authority_error(error, None))?;
        let current_revision = current.as_ref().map(|head| head.revision);
        let mut workflow = current
            .map(|head| head.workflow)
            .unwrap_or_else(|| empty_workflow(context.project_id()));
        let mut aliases = Vec::new();
        let mut readiness_blockers = Vec::new();
        for patch in patches {
            let result = apply_workflow_patch(&self.registry, &workflow, patch)
                .map_err(|error| patch_error(error, current_revision))?;
            workflow = result.workflow;
            aliases.extend(
                result
                    .aliases
                    .into_iter()
                    .map(|(alias, node_id)| WorkflowAliasDto { alias, node_id }),
            );
            readiness_blockers = result.readiness_blockers;
        }
        let request_hash = request_hash(expected_revision, &patches)
            .map_err(|error| hash_error(error, current_revision))?;
        let committed = self
            .authority
            .apply(WorkflowCommitRequest::new(
                context.project_id(),
                expected_revision,
                context.request_id(),
                request_hash,
                workflow,
            ))
            .map_err(|error| authority_error(error, current_revision))?;
        to_output_parts(committed, aliases, readiness_blockers)
    }
}

fn hash_error(error: serde_json::Error, current_revision: Option<u64>) -> WorkflowApplyPatchError {
    WorkflowApplyPatchError::new(
        "PATCH_HASH_FAILED",
        "/operations",
        None,
        error.to_string(),
        current_revision,
    )
}
