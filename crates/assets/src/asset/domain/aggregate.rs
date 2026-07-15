//! Project-local Asset aggregate and approved content transitions.

use projects::project::domain::ProjectId;

use super::{
    AssetContentDescriptor, AssetContentFinalizationId, AssetContentMissingReason, AssetCreatedAt,
    AssetDisplayName, AssetDomainError, AssetId, AssetManagedContentState, AssetMediaFacts,
    AssetMediaKind, AssetOrigin,
};

/// One Project-local logical Image, Video, or Audio item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetAggregate {
    id: AssetId,
    project_id: ProjectId,
    media_kind: AssetMediaKind,
    content_state: AssetManagedContentState,
    media_facts: AssetMediaFacts,
    origin: AssetOrigin,
    display_name: AssetDisplayName,
    created_at: AssetCreatedAt,
}

impl AssetAggregate {
    /// Creates one new Pending Asset after checking all immutable fields agree.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_pending(
        id: AssetId,
        project_id: ProjectId,
        media_kind: AssetMediaKind,
        descriptor: AssetContentDescriptor,
        finalization_id: AssetContentFinalizationId,
        media_facts: AssetMediaFacts,
        origin: AssetOrigin,
        display_name: AssetDisplayName,
        created_at: AssetCreatedAt,
    ) -> Result<Self, AssetDomainError> {
        Self::try_restore(
            id,
            project_id,
            media_kind,
            AssetManagedContentState::Pending { descriptor, finalization_id },
            media_facts,
            origin,
            display_name,
            created_at,
        )
    }

    /// Restores an Asset only when descriptor, facts, and outer media kind agree.
    #[allow(clippy::too_many_arguments)]
    pub fn try_restore(
        id: AssetId,
        project_id: ProjectId,
        media_kind: AssetMediaKind,
        content_state: AssetManagedContentState,
        media_facts: AssetMediaFacts,
        origin: AssetOrigin,
        display_name: AssetDisplayName,
        created_at: AssetCreatedAt,
    ) -> Result<Self, AssetDomainError> {
        if content_state.descriptor().media_kind() != media_kind {
            return Err(AssetDomainError::InvalidDescriptor);
        }
        if media_facts.media_kind() != media_kind {
            return Err(AssetDomainError::InvalidMediaFacts);
        }
        Ok(Self {
            id,
            project_id,
            media_kind,
            content_state,
            media_facts,
            origin,
            display_name,
            created_at,
        })
    }

    /// Approves Pending-to-Available only for the exact durable finalization identity.
    pub fn mark_pending_content_available(
        &mut self,
        finalization_id: AssetContentFinalizationId,
    ) -> Result<(), AssetDomainError> {
        let AssetManagedContentState::Pending {
            descriptor,
            finalization_id: expected_finalization_id,
        } = &self.content_state
        else {
            return Err(AssetDomainError::InvalidTransition);
        };
        if *expected_finalization_id != finalization_id {
            return Err(AssetDomainError::FinalizationIdentityMismatch);
        }
        self.content_state = AssetManagedContentState::Available { descriptor: descriptor.clone() };
        Ok(())
    }

    /// Approves Pending/Available-to-Missing while preserving the exact expected descriptor.
    pub fn mark_content_missing(
        &mut self,
        reason: AssetContentMissingReason,
    ) -> Result<(), AssetDomainError> {
        let expected = match &self.content_state {
            AssetManagedContentState::Pending { descriptor, .. }
            | AssetManagedContentState::Available { descriptor } => descriptor.clone(),
            AssetManagedContentState::Missing { .. } => {
                return Err(AssetDomainError::InvalidTransition);
            }
        };
        self.content_state = AssetManagedContentState::Missing { expected, reason };
        Ok(())
    }

    /// Approves Missing-to-Available only for the exact expected immutable content.
    pub fn restore_missing_content(
        &mut self,
        recovered: AssetContentDescriptor,
    ) -> Result<(), AssetDomainError> {
        let AssetManagedContentState::Missing { expected, .. } = &self.content_state else {
            return Err(AssetDomainError::InvalidTransition);
        };
        if expected != &recovered {
            return Err(AssetDomainError::InvalidTransition);
        }
        self.content_state = AssetManagedContentState::Available { descriptor: recovered };
        Ok(())
    }

    /// Returns the logical Asset identity.
    #[must_use]
    pub const fn id(&self) -> AssetId {
        self.id
    }
    /// Returns the authoritative owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    /// Returns the immutable media kind.
    #[must_use]
    pub const fn media_kind(&self) -> AssetMediaKind {
        self.media_kind
    }
    /// Returns current managed-content state.
    #[must_use]
    pub const fn content_state(&self) -> &AssetManagedContentState {
        &self.content_state
    }
    /// Returns immutable inspected technical facts.
    #[must_use]
    pub const fn media_facts(&self) -> AssetMediaFacts {
        self.media_facts
    }
    /// Returns immutable provenance.
    #[must_use]
    pub const fn origin(&self) -> &AssetOrigin {
        &self.origin
    }
    /// Returns immutable display name.
    #[must_use]
    pub const fn display_name(&self) -> &AssetDisplayName {
        &self.display_name
    }
    /// Returns immutable creation time.
    #[must_use]
    pub const fn created_at(&self) -> AssetCreatedAt {
        self.created_at
    }
}
