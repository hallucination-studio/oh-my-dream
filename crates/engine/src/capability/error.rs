use super::{CapabilityRef, CapabilitySelector};
use thiserror::Error;

/// Errors raised while constructing or resolving the exact registry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityRegistryError {
    /// A registration did not provide a non-empty id and version.
    #[error("capability id and version must be non-empty")]
    EmptyReference,
    /// A capability ref was registered more than once.
    #[error("duplicate capability reference `{reference:?}`")]
    DuplicateReference { reference: CapabilityRef },
    /// An exact capability ref was not registered.
    #[error("unknown capability reference `{reference:?}`")]
    UnknownReference { reference: CapabilityRef },
    /// The registration's empty-input normalization disagrees with its defaults.
    #[error("invalid default params for capability `{reference:?}`: {reason}")]
    InvalidDefaultParams {
        reference: CapabilityRef,
        reason: String,
    },
    /// A selector-aware registration did not declare its selector.
    #[error("capability `{reference:?}` must declare a selector")]
    MissingSelector { reference: CapabilityRef },
    /// A selector did not provide both a modality and mode.
    #[error("capability selector type_id and mode must be non-empty: `{selector:?}`")]
    EmptySelector { selector: CapabilitySelector },
    /// A selector was already bound to a different stable capability id.
    #[error(
        "capability selector `{selector:?}` is bound to `{registered_id}` and cannot select `{attempted_id}`"
    )]
    SelectorRebind {
        selector: CapabilitySelector,
        registered_id: String,
        attempted_id: String,
    },
}
