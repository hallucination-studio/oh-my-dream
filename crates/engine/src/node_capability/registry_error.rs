//! Immutable node-capability registry failures.

use thiserror::Error;

use super::NodeCapabilityContractRef;

/// Immutable-registry construction or lookup failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NodeCapabilityRegistryError {
    /// Two implementations declared the same exact ref.
    #[error("duplicate node capability contract ref: {contract_ref}")]
    DuplicateContractRef {
        /// Duplicated exact contract identity.
        contract_ref: NodeCapabilityContractRef,
    },
    /// No active implementation had the requested exact ref.
    #[error("node capability contract is not registered: {contract_ref}")]
    ContractNotRegistered {
        /// Missing exact contract identity.
        contract_ref: NodeCapabilityContractRef,
    },
}
