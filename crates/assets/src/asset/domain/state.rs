//! Managed immutable-content availability state.

use super::{AssetContentDescriptor, AssetContentFinalizationId};

/// Exact reason expected managed content is absent.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AssetContentMissingReason {
    /// Durable finalization source no longer exists.
    FinalizationSourceMissing,
    /// Previously published immutable managed content is absent.
    ManagedContentMissing,
}

/// Closed managed-content lifecycle owned by the Asset aggregate.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum AssetManagedContentState {
    /// Descriptor is committed but immutable bytes are not yet published.
    Pending {
        /// Exact content expected from finalization.
        descriptor: AssetContentDescriptor,
        /// Durable finalization identity.
        finalization_id: AssetContentFinalizationId,
    },
    /// Exact immutable bytes are published and available.
    Available {
        /// Published exact content descriptor.
        descriptor: AssetContentDescriptor,
    },
    /// Exact expected content is currently absent.
    Missing {
        /// Descriptor that must be recovered exactly.
        expected: AssetContentDescriptor,
        /// Structured absence reason.
        reason: AssetContentMissingReason,
    },
}

impl AssetManagedContentState {
    /// Returns the exact immutable descriptor regardless of lifecycle state.
    #[must_use]
    pub const fn descriptor(&self) -> &AssetContentDescriptor {
        match self {
            Self::Pending { descriptor, .. } | Self::Available { descriptor } => descriptor,
            Self::Missing { expected, .. } => expected,
        }
    }
}
