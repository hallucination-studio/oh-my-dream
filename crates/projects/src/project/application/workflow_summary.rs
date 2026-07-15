//! Minimal Project-owned projection of the optional current Workflow.

/// Opaque translated Workflow identity used only by Project open.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ProjectWorkflowIdBoundaryValue(String);

impl ProjectWorkflowIdBoundaryValue {
    /// Validates a non-empty Workflow identity of at most 128 UTF-8 bytes.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty() && value.len() <= 128).then_some(Self(value))
    }

    /// Returns the opaque translated identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-zero translated Workflow revision used only by Project open.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectWorkflowRevisionBoundaryValue(u64);

impl ProjectWorkflowRevisionBoundaryValue {
    /// Validates a non-zero translated Workflow revision.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the translated Workflow revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Project's intentionally lossy current-Workflow readiness projection.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ProjectWorkflowReadinessSummary {
    /// The authoritative Workflow issue set was empty.
    Ready,
    /// The authoritative Workflow issue set was non-empty.
    Blocked,
}

/// Minimal current-Workflow summary returned while opening a Project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectWorkflowSummary {
    /// Opaque translated Workflow identity.
    pub workflow_id: ProjectWorkflowIdBoundaryValue,
    /// Non-zero translated Workflow revision.
    pub workflow_revision: ProjectWorkflowRevisionBoundaryValue,
    /// Lossy Ready or Blocked projection.
    pub readiness: ProjectWorkflowReadinessSummary,
}
