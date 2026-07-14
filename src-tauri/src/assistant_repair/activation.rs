use serde::{Deserialize, Serialize};

/// Factual lifecycle input that may start a new Agent turn after a failed run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairActivation {
    pub kind: String,
    pub project_id: String,
    pub session_id: String,
    pub run_id: String,
    pub workflow_revision: u64,
    pub reason: String,
}

impl RepairActivation {
    pub(super) fn failed(
        project_id: &str,
        run_id: &str,
        workflow_revision: u64,
        reason: String,
    ) -> Self {
        Self {
            kind: "workflow_run_failed".to_owned(),
            project_id: project_id.to_owned(),
            session_id: format!("project:{project_id}"),
            run_id: run_id.to_owned(),
            workflow_revision,
            reason,
        }
    }
}
