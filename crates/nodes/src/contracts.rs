//! Registration-derived capability contract and presentation projections.

use engine::{CapabilityContract, CapabilityPresentation, CapabilityRef, CapabilitySelector, NodeRegistry};
use thiserror::Error;

/// One exact execution contract paired with its non-authoritative presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityProjection {
    /// Workflow modality and mode selecting this exact registration.
    pub selector: CapabilitySelector,
    /// Immutable execution semantics.
    pub contract: CapabilityContract,
    /// Mutable display metadata derived from the same registration.
    pub presentation: CapabilityPresentation,
}

/// Projects all exact registrations in stable ref order.
pub fn project_capabilities(
    registry: &NodeRegistry,
) -> Result<Vec<CapabilityProjection>, CapabilityProjectionError> {
    registry
        .capability_refs()
        .into_iter()
        .map(|reference| project_capability(registry, reference))
        .collect()
}

/// Projects one exact registration into contract and presentation data.
pub fn project_capability(
    registry: &NodeRegistry,
    reference: &CapabilityRef,
) -> Result<CapabilityProjection, CapabilityProjectionError> {
    let registration = registry.capability(reference).map_err(|source| {
        CapabilityProjectionError::MissingRegistration {
            reference: reference.clone(),
            message: source.to_string(),
        }
    })?;
    Ok(CapabilityProjection {
        selector: registration.selector().cloned().ok_or_else(|| {
            CapabilityProjectionError::MissingSelector { reference: reference.clone() }
        })?,
        contract: registration.contract().clone(),
        presentation: registration.presentation().clone(),
    })
}

/// Projection failure indicating a registry invariant was broken.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityProjectionError {
    /// The registry exposed a ref that could not be looked up again.
    #[error("capability `{reference:?}` disappeared during projection: {message}")]
    MissingRegistration { reference: engine::CapabilityRef, message: String },
    /// A discoverable registration did not declare its Workflow selector.
    #[error("capability `{reference:?}` has no Workflow selector")]
    MissingSelector { reference: engine::CapabilityRef },
}
