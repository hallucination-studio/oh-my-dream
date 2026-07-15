//! Project aggregate and its only MVP state transition.

use super::{
    ProjectCreatedAt, ProjectDomainError, ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt,
};

/// One durable creative workspace identity, name, and revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectAggregate {
    id: ProjectId,
    name: ProjectName,
    revision: ProjectRevision,
    created_at: ProjectCreatedAt,
    updated_at: ProjectUpdatedAt,
}

impl ProjectAggregate {
    /// Creates a Project at revision one with equal creation and update times.
    #[must_use]
    pub fn create(id: ProjectId, name: ProjectName, created_at: ProjectCreatedAt) -> Self {
        Self {
            id,
            name,
            revision: ProjectRevision::initial(),
            created_at,
            updated_at: created_at.into(),
        }
    }

    /// Restores a persisted Project after validating aggregate-level time ordering.
    pub fn restore(
        id: ProjectId,
        name: ProjectName,
        revision: ProjectRevision,
        created_at: ProjectCreatedAt,
        updated_at: ProjectUpdatedAt,
    ) -> Result<Self, ProjectDomainError> {
        if updated_at.get() < created_at.get() {
            return Err(ProjectDomainError::ProjectTimestampOutOfRange);
        }
        Ok(Self { id, name, revision, created_at, updated_at })
    }

    /// Renames this Project and advances its revision and monotonic update time.
    pub fn rename(
        &mut self,
        name: ProjectName,
        observed_at: ProjectUpdatedAt,
    ) -> Result<(), ProjectDomainError> {
        if self.name == name {
            return Err(ProjectDomainError::ProjectNameUnchanged);
        }
        let revision = self.revision.next()?;
        let minimum = self
            .updated_at
            .get()
            .checked_add(1)
            .ok_or(ProjectDomainError::ProjectTimestampOverflow)?;
        self.name = name;
        self.revision = revision;
        self.updated_at = ProjectUpdatedAt::new(observed_at.get().max(minimum))?;
        Ok(())
    }

    /// Returns the Project identity.
    #[must_use]
    pub const fn id(&self) -> ProjectId {
        self.id
    }

    /// Returns the normalized Project name.
    #[must_use]
    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    /// Returns the current Project revision.
    #[must_use]
    pub const fn revision(&self) -> ProjectRevision {
        self.revision
    }

    /// Returns the immutable creation time.
    #[must_use]
    pub const fn created_at(&self) -> ProjectCreatedAt {
        self.created_at
    }

    /// Returns the last update time.
    #[must_use]
    pub const fn updated_at(&self) -> ProjectUpdatedAt {
        self.updated_at
    }
}
