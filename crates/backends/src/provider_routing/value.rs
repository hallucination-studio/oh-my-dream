use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
};
use nodes::{GenerationProfileAvailabilityState, GenerationProfileCatalog, GenerationProfileRef};
use thiserror::Error;

/// Current route state before profile-level observation metadata is attached.
pub type GenerationProviderRouteAvailability = GenerationProfileAvailabilityState;

/// Stable process-local identity for one configured provider route.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationProviderRouteId(String);

impl GenerationProviderRouteId {
    /// Wraps the exact configuration-owned route identity without interpreting it.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the exact configured route identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid exact-operation provider router configuration.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum GenerationProviderRouterConstructionError {
    /// The input repeats one exact Generation Profile.
    #[error("generation provider profile mapping is duplicated")]
    DuplicateProfileMapping,
    /// Two configured entries expose the same concrete route identity.
    #[error("generation provider route identity is duplicated")]
    DuplicateRouteId,
    /// A configured profile is absent from the frozen catalog.
    #[error("generation provider profile is unknown")]
    UnknownProfile,
    /// A configured profile belongs to another exact operation.
    #[error("generation provider profile is incompatible")]
    IncompatibleProfile,
    /// A frozen internal router value could not be constructed.
    #[error("generation provider router configuration is invalid")]
    InvalidRouteMap,
}

pub(super) fn validate_routes<R: ?Sized>(
    routes: impl IntoIterator<Item = (GenerationProfileRef, Arc<R>)>,
    capability_id: &str,
    route_id: impl Fn(&R) -> GenerationProviderRouteId,
) -> Result<BTreeMap<GenerationProfileRef, Arc<R>>, GenerationProviderRouterConstructionError> {
    let mut routes_by_profile = BTreeMap::new();
    let mut route_ids = BTreeSet::new();
    for (profile_ref, route) in routes {
        if routes_by_profile.contains_key(&profile_ref) {
            return Err(GenerationProviderRouterConstructionError::DuplicateProfileMapping);
        }
        if !route_ids.insert(route_id(&route)) {
            return Err(GenerationProviderRouterConstructionError::DuplicateRouteId);
        }
        routes_by_profile.insert(profile_ref, route);
    }

    let capability_ref = NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(capability_id)
            .map_err(|_| GenerationProviderRouterConstructionError::InvalidRouteMap)?,
        NodeCapabilityContractVersion::new(1, 0)
            .map_err(|_| GenerationProviderRouterConstructionError::InvalidRouteMap)?,
    );
    let catalog = GenerationProfileCatalog::frozen_mvp()
        .map_err(|_| GenerationProviderRouterConstructionError::InvalidRouteMap)?;
    for profile_ref in routes_by_profile.keys() {
        let definition = catalog
            .find_generation_profile(profile_ref)
            .map_err(|_| GenerationProviderRouterConstructionError::UnknownProfile)?;
        if !definition.compatible_capabilities().contains(&capability_ref) {
            return Err(GenerationProviderRouterConstructionError::IncompatibleProfile);
        }
    }
    Ok(routes_by_profile)
}

pub(super) fn no_configured_route_failure()
-> Result<NodeCapabilityProviderFailure, GenerationProviderRouterConstructionError> {
    NodeCapabilityProviderFailure::try_new(
        NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        false,
        Instant::now(),
        None,
    )
    .map_err(|_| GenerationProviderRouterConstructionError::InvalidRouteMap)
}
