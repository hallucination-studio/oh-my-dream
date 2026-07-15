//! Bounded Project keyset-list values.

use super::ProjectApplicationError;
use crate::project::domain::{ProjectAggregate, ProjectId, ProjectUpdatedAt};

/// Validated maximum number of Projects returned in one page.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectListLimit(u16);

impl ProjectListLimit {
    /// Validates the frozen `1..=100` Project page bound.
    pub fn new(value: u16) -> Result<Self, ProjectApplicationError> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ProjectApplicationError::ProjectListLimitOutOfBounds { requested_limit: value })
        }
    }

    /// Returns the validated page limit.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Exclusive `(updated_at, project_id)` keyset position.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectListCursor {
    /// Update time of the last returned Project.
    pub updated_at: ProjectUpdatedAt,
    /// Identity of the last returned Project.
    pub project_id: ProjectId,
}

/// One bounded Project list request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectListQuery {
    /// Maximum number of Projects to return.
    pub limit: ProjectListLimit,
    /// Optional exclusive keyset position.
    pub cursor: Option<ProjectListCursor>,
}

/// One deterministic Project page ordered by update time and identity descending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectListPage {
    /// At most the requested number of Projects.
    pub projects: Vec<ProjectAggregate>,
    /// Cursor for another page only when another row exists.
    pub next_cursor: Option<ProjectListCursor>,
}
