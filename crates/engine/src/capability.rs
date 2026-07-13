//! Pure capability identity, contract, and exact-resolution primitives.

use crate::node::{Node, NodeRunError};
use crate::port::PortCardinality;
use crate::registry::{NodeFactory, NodeParams};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Default contract version used by the first workflow capability set.
pub const DEFAULT_CAPABILITY_VERSION: &str = "1.0";

/// Exact identity of one executable capability contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityRef {
    /// Stable capability identifier, also persisted as Workflow `type`.
    pub id: String,
    /// Exact semantic contract version.
    pub version: String,
}

impl CapabilityRef {
    /// Creates an exact capability reference.
    #[must_use]
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self { id: id.into(), version: version.into() }
    }
}

/// Execution-relevant port metadata owned by a capability contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPort {
    /// Stable named port.
    pub name: String,
    /// Value type carried by the port.
    pub port_type: crate::PortType,
    /// Port cardinality.
    pub cardinality: PortCardinality,
    /// Whether an input must be connected or have a default.
    pub required: bool,
}

impl CapabilityPort {
    /// Creates a single-value input port.
    #[must_use]
    pub fn input(name: impl Into<String>, port_type: crate::PortType, required: bool) -> Self {
        Self { name: name.into(), port_type, cardinality: PortCardinality::One, required }
    }

    /// Creates a single-value output port.
    #[must_use]
    pub fn output(name: impl Into<String>, port_type: crate::PortType) -> Self {
        Self::input(name, port_type, false)
    }

    /// Changes an input or output to ordered-many cardinality.
    #[must_use]
    pub fn with_cardinality(mut self, cardinality: PortCardinality) -> Self {
        self.cardinality = cardinality;
        self
    }
}

/// Policy-relevant side effect classification for one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEffect {
    /// Deterministic local transformation with no external effect.
    Pure,
    /// Provider, filesystem, or other external effect.
    External,
}

/// Immutable execution contract derived from one capability registration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityContract {
    /// Exact identity represented by this contract.
    pub reference: CapabilityRef,
    /// Named input ports and their cardinalities.
    pub inputs: Vec<CapabilityPort>,
    /// Named output ports and their cardinalities.
    pub outputs: Vec<CapabilityPort>,
    /// Generated boundary schema for the params object.
    pub params_schema: serde_json::Value,
    /// Canonical normalized params used when no params were supplied.
    pub default_params: NodeParams,
    /// Effects used by policy and approval layers.
    pub effects: Vec<CapabilityEffect>,
}

impl CapabilityContract {
    /// Creates an immutable capability contract.
    #[must_use]
    pub fn new(
        reference: CapabilityRef,
        inputs: Vec<CapabilityPort>,
        outputs: Vec<CapabilityPort>,
        params_schema: serde_json::Value,
        default_params: NodeParams,
        effects: Vec<CapabilityEffect>,
    ) -> Self {
        Self { reference, inputs, outputs, params_schema, default_params, effects }
    }
}

/// One capability's normalization policy and executable node factory.
pub struct CapabilityRegistration {
    contract: CapabilityContract,
    normalizer: CapabilityNormalizer,
    factory: NodeFactory,
}

/// Normalizes and validates raw JSON params into canonical params.
pub type CapabilityNormalizer =
    Box<dyn Fn(&NodeParams) -> Result<NodeParams, NodeRunError> + Send + Sync>;

impl CapabilityRegistration {
    /// Binds one contract, normalizer, and executable-node factory.
    #[must_use]
    pub fn new(
        contract: CapabilityContract,
        normalizer: CapabilityNormalizer,
        factory: NodeFactory,
    ) -> Self {
        Self { contract, normalizer, factory }
    }

    /// Returns the exact capability reference.
    #[must_use]
    pub fn reference(&self) -> &CapabilityRef {
        &self.contract.reference
    }

    /// Returns the immutable execution contract.
    #[must_use]
    pub fn contract(&self) -> &CapabilityContract {
        &self.contract
    }

    /// Normalizes raw params once at the registration boundary.
    pub fn normalize_params(&self, params: &NodeParams) -> Result<NodeParams, NodeRunError> {
        (self.normalizer)(params)
    }

    /// Constructs an executable node from canonical params.
    pub fn instantiate(&self, params: &NodeParams) -> Result<Box<dyn Node>, NodeRunError> {
        (self.factory)(params)
    }
}

/// Exact capability registry used by the pure engine boundary.
#[derive(Default)]
pub struct CapabilityRegistry {
    registrations: BTreeMap<CapabilityRef, CapabilityRegistration>,
    current_versions: BTreeMap<String, String>,
}

impl CapabilityRegistry {
    /// Registers a capability and marks its version as current for discovery.
    pub fn register_current(
        &mut self,
        registration: CapabilityRegistration,
    ) -> Result<(), CapabilityRegistryError> {
        let reference = registration.reference().clone();
        self.register(registration)?;
        self.current_versions.insert(reference.id.clone(), reference.version.clone());
        Ok(())
    }

    /// Registers a capability without changing the current discovery version.
    pub fn register(
        &mut self,
        registration: CapabilityRegistration,
    ) -> Result<(), CapabilityRegistryError> {
        let reference = registration.reference().clone();
        if reference.id.is_empty() || reference.version.is_empty() {
            return Err(CapabilityRegistryError::EmptyReference);
        }
        if self.registrations.contains_key(&reference) {
            return Err(CapabilityRegistryError::DuplicateReference { reference });
        }
        let normalized_defaults =
            registration.normalize_params(&NodeParams::new()).map_err(|source| {
                CapabilityRegistryError::InvalidDefaultParams {
                    reference: reference.clone(),
                    reason: source.to_string(),
                }
            })?;
        if normalized_defaults != registration.contract.default_params {
            return Err(CapabilityRegistryError::InvalidDefaultParams {
                reference,
                reason: "normalizer output does not match default_params".to_owned(),
            });
        }
        self.registrations.insert(reference, registration);
        Ok(())
    }

    /// Marks an already registered exact version as current for new-node search.
    pub fn mark_current(
        &mut self,
        reference: &CapabilityRef,
    ) -> Result<(), CapabilityRegistryError> {
        if !self.registrations.contains_key(reference) {
            return Err(CapabilityRegistryError::UnknownReference { reference: reference.clone() });
        }
        self.current_versions.insert(reference.id.clone(), reference.version.clone());
        Ok(())
    }

    /// Resolves only the requested exact `{id, version}`.
    pub fn resolve(
        &self,
        reference: &CapabilityRef,
    ) -> Result<&CapabilityRegistration, CapabilityRegistryError> {
        self.registrations.get(reference).ok_or_else(|| CapabilityRegistryError::UnknownReference {
            reference: reference.clone(),
        })
    }

    /// Returns the current exact ref for new-node discovery.
    #[must_use]
    pub fn current(&self, id: &str) -> Option<CapabilityRef> {
        self.current_versions.get(id).map(|version| CapabilityRef::new(id, version))
    }

    /// Returns whether any exact version exists for an id.
    #[must_use]
    pub fn contains_id(&self, id: &str) -> bool {
        self.registrations.keys().any(|reference| reference.id == id)
    }

    /// Returns all exact refs in stable order.
    #[must_use]
    pub fn references(&self) -> Vec<&CapabilityRef> {
        self.registrations.keys().collect()
    }
}

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
    InvalidDefaultParams { reference: CapabilityRef, reason: String },
}
