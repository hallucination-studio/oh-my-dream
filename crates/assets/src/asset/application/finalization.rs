//! Asset finalization facts and atomic transaction commands.

use crate::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentFinalizationId, AssetContentMissingReason,
    AssetCreatedAt, AssetId, AssetManagedContentState,
};

use super::{AssetApplicationError, AssetStagedContentRef};

/// Durable facts required to publish one exact staged object.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetContentFinalization {
    finalization_id: AssetContentFinalizationId,
    asset_id: AssetId,
    descriptor: AssetContentDescriptor,
    staged_content_ref: AssetStagedContentRef,
    created_at: AssetCreatedAt,
}

impl AssetContentFinalization {
    /// Creates finalization facts from already-validated values.
    #[must_use]
    pub const fn new(
        finalization_id: AssetContentFinalizationId,
        asset_id: AssetId,
        descriptor: AssetContentDescriptor,
        staged_content_ref: AssetStagedContentRef,
        created_at: AssetCreatedAt,
    ) -> Self {
        Self { finalization_id, asset_id, descriptor, staged_content_ref, created_at }
    }

    /// Returns the idempotent finalization identity.
    #[must_use]
    pub const fn finalization_id(&self) -> AssetContentFinalizationId {
        self.finalization_id
    }

    /// Returns the Asset receiving the exact content.
    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Returns the expected exact content descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &AssetContentDescriptor {
        &self.descriptor
    }

    /// Returns the opaque staged source identity.
    #[must_use]
    pub const fn staged_content_ref(&self) -> &AssetStagedContentRef {
        &self.staged_content_ref
    }

    /// Returns the owning aggregate's creation time.
    #[must_use]
    pub const fn created_at(&self) -> AssetCreatedAt {
        self.created_at
    }
}

/// Closed post-commit request to finalize one exact staged object.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetFinalizeContentEffect {
    finalization_id: AssetContentFinalizationId,
}

impl AssetFinalizeContentEffect {
    /// Creates an effect for one committed finalization.
    #[must_use]
    pub const fn new(finalization_id: AssetContentFinalizationId) -> Self {
        Self { finalization_id }
    }

    /// Returns the exact finalization identity.
    #[must_use]
    pub const fn finalization_id(self) -> AssetContentFinalizationId {
        self.finalization_id
    }
}

/// Exact atomic input for inserting one Pending Asset and finalization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetCommitPendingContentCommand {
    asset: AssetAggregate,
    finalization: AssetContentFinalization,
    effect: AssetFinalizeContentEffect,
}

impl AssetCommitPendingContentCommand {
    /// Creates a command only when every Pending identity and descriptor agrees.
    pub fn try_new(
        asset: AssetAggregate,
        finalization: AssetContentFinalization,
        effect: AssetFinalizeContentEffect,
    ) -> Result<Self, AssetApplicationError> {
        let AssetManagedContentState::Pending { descriptor, finalization_id } =
            asset.content_state()
        else {
            return Err(AssetApplicationError::IdentityConflict);
        };
        let identities_agree = asset.id() == finalization.asset_id()
            && asset.created_at() == finalization.created_at()
            && descriptor == finalization.descriptor()
            && *finalization_id == finalization.finalization_id()
            && *finalization_id == effect.finalization_id();
        if !identities_agree {
            return Err(AssetApplicationError::IdentityConflict);
        }
        Ok(Self { asset, finalization, effect })
    }

    /// Returns the complete Pending aggregate.
    #[must_use]
    pub const fn asset(&self) -> &AssetAggregate {
        &self.asset
    }

    /// Returns the matching finalization facts.
    #[must_use]
    pub const fn finalization(&self) -> &AssetContentFinalization {
        &self.finalization
    }

    /// Returns the matching closed Asset effect.
    #[must_use]
    pub const fn effect(&self) -> AssetFinalizeContentEffect {
        self.effect
    }
}

/// Result of atomically committing a Workflow-node Pending Asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetCommitWorkflowNodeOutputPendingResult {
    /// The new output binding, Asset, finalization, and effect were committed.
    Committed,
    /// The exact output key was already bound to this Asset.
    OutputKeyAlreadyBound {
        /// Existing Asset returned for application-owned replay comparison.
        asset: Box<AssetAggregate>,
    },
}

/// Exact atomic input for completing one finalization as Available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetCommitFinalizedContentAvailableCommand {
    asset: AssetAggregate,
    finalization_id: AssetContentFinalizationId,
}

impl AssetCommitFinalizedContentAvailableCommand {
    /// Creates a command only for an Available aggregate.
    pub fn try_new(
        asset: AssetAggregate,
        finalization_id: AssetContentFinalizationId,
    ) -> Result<Self, AssetApplicationError> {
        if !matches!(asset.content_state(), AssetManagedContentState::Available { .. }) {
            return Err(AssetApplicationError::IdentityConflict);
        }
        Ok(Self { asset, finalization_id })
    }

    /// Returns the already-approved Available aggregate.
    #[must_use]
    pub const fn asset(&self) -> &AssetAggregate {
        &self.asset
    }

    /// Returns the finalization being completed.
    #[must_use]
    pub const fn finalization_id(&self) -> AssetContentFinalizationId {
        self.finalization_id
    }
}

/// Exact atomic input for persisting one approved Missing transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetCommitContentMissingCommand {
    asset: AssetAggregate,
    finalization_id: Option<AssetContentFinalizationId>,
}

impl AssetCommitContentMissingCommand {
    /// Creates a command only when Missing reason and finalization presence agree.
    pub fn try_new(
        asset: AssetAggregate,
        finalization_id: Option<AssetContentFinalizationId>,
    ) -> Result<Self, AssetApplicationError> {
        let AssetManagedContentState::Missing { reason, .. } = asset.content_state() else {
            return Err(AssetApplicationError::IdentityConflict);
        };
        let identity_shape_agrees = matches!(
            (reason, finalization_id),
            (AssetContentMissingReason::FinalizationSourceMissing, Some(_))
                | (AssetContentMissingReason::ManagedContentMissing, None)
        );
        if !identity_shape_agrees {
            return Err(AssetApplicationError::IdentityConflict);
        }
        Ok(Self { asset, finalization_id })
    }

    /// Returns the already-approved Missing aggregate.
    #[must_use]
    pub const fn asset(&self) -> &AssetAggregate {
        &self.asset
    }

    /// Returns the Pending-origin finalization, when one must be completed.
    #[must_use]
    pub const fn finalization_id(&self) -> Option<AssetContentFinalizationId> {
        self.finalization_id
    }
}
