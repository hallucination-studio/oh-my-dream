//! Key-first recovery for one deterministic Workflow node output.

use std::sync::Arc;

use crate::asset::domain::{AssetAggregate, AssetManagedContentState, AssetNodeOutputKey};
use crate::asset::interfaces::AssetRepositoryInterface;

use super::{AssetApplicationError, AssetContentFinalization};

/// Durable state observed for one exact node-output key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetNodeOutputRecovery {
    /// The exact output Asset is already available.
    Available(Box<AssetAggregate>),
    /// The exact output Asset has an unfinished durable finalization.
    Pending {
        /// Durable finalization that owns the pending publication.
        finalization: AssetContentFinalization,
    },
    /// No recoverable Asset publication exists for this key.
    SourceRequired,
}

/// Inspects deterministic node-output recovery state without accepting source bytes.
pub struct AssetRecoverNodeOutputUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
}

impl AssetRecoverNodeOutputUseCase {
    /// Creates the key-first recovery use case.
    #[must_use]
    pub fn new(repository: Arc<dyn AssetRepositoryInterface>) -> Self {
        Self { repository }
    }

    /// Returns the exact durable recovery state for one node-output key.
    pub async fn recover_asset_node_output(
        &self,
        output_key: AssetNodeOutputKey,
    ) -> Result<AssetNodeOutputRecovery, AssetApplicationError> {
        let Some(asset) = self.repository.find_asset_by_node_output_key(output_key).await? else {
            return Ok(AssetNodeOutputRecovery::SourceRequired);
        };
        let (descriptor, finalization_id) = match asset.content_state() {
            AssetManagedContentState::Available { .. } => {
                return Ok(AssetNodeOutputRecovery::Available(Box::new(asset)));
            }
            AssetManagedContentState::Missing { .. } => {
                return Ok(AssetNodeOutputRecovery::SourceRequired);
            }
            AssetManagedContentState::Pending { descriptor, finalization_id } => {
                (descriptor, finalization_id)
            }
        };
        let Some(finalization) =
            self.repository.find_asset_content_finalization(*finalization_id).await?
        else {
            return Ok(AssetNodeOutputRecovery::SourceRequired);
        };
        if finalization.asset_id() != asset.id() || finalization.descriptor() != descriptor {
            return Err(AssetApplicationError::FinalizationFailed);
        }
        Ok(AssetNodeOutputRecovery::Pending { finalization })
    }
}
