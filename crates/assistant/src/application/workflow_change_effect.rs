//! Post-commit intent for one approved Workflow change.

use crate::domain::AssistantWorkflowChangeId;

/// Idempotent intent to apply one exact approved Assistant Workflow change.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssistantApplyWorkflowChangeEffect {
    workflow_change_id: AssistantWorkflowChangeId,
}

impl AssistantApplyWorkflowChangeEffect {
    /// Creates the closed effect from its owning change identity.
    #[must_use]
    pub const fn new(workflow_change_id: AssistantWorkflowChangeId) -> Self {
        Self { workflow_change_id }
    }

    /// Returns the exact approved change identity.
    #[must_use]
    pub const fn workflow_change_id(self) -> AssistantWorkflowChangeId {
        self.workflow_change_id
    }
}
