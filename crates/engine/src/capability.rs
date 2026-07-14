//! Pure capability identity, contract, and exact-resolution primitives.

use crate::node::{Node, NodeRunError};
use crate::port::PortCardinality;
use crate::registry::{NodeFactory, NodeParams};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod error;

pub use error::CapabilityRegistryError;

/// Default contract version used by the first workflow capability set.
pub const DEFAULT_CAPABILITY_VERSION: &str = "1.0";

/// Exact identity of one executable capability contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// Workflow-facing discriminator that selects one exact capability family.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySelector {
    /// Output modality persisted as Workflow `type`.
    pub type_id: String,
    /// Mode persisted in Workflow `params.mode`.
    pub mode: String,
}

impl CapabilitySelector {
    /// Creates a modality and mode selector.
    #[must_use]
    pub fn new(type_id: impl Into<String>, mode: impl Into<String>) -> Self {
        Self { type_id: type_id.into(), mode: mode.into() }
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

/// Mutable, non-authoritative display metadata derived from a registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPresentation {
    /// Short label shown in palettes and node headers.
    pub label: String,
    /// User-facing description of the capability.
    pub description: String,
    /// Presentation grouping, not an execution identity.
    pub category: String,
    /// Search terms used by discovery and UI filtering.
    pub search_terms: Vec<String>,
}

impl CapabilityPresentation {
    /// Creates display metadata for one registration.
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        search_terms: Vec<String>,
    ) -> Self {
        Self {
            label: label.into(),
            description: description.into(),
            category: category.into(),
            search_terms,
        }
    }
}

/// One capability's normalization policy and executable node factory.
pub struct CapabilityRegistration {
    contract: CapabilityContract,
    presentation: CapabilityPresentation,
    selector: Option<CapabilitySelector>,
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
        presentation: CapabilityPresentation,
        normalizer: CapabilityNormalizer,
        factory: NodeFactory,
    ) -> Self {
        Self { contract, presentation, selector: None, normalizer, factory }
    }

    /// Declares the Workflow modality and mode that select this exact registration.
    #[must_use]
    pub fn with_selector(mut self, selector: CapabilitySelector) -> Self {
        self.selector = Some(selector);
        self
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

    /// Returns non-authoritative display metadata for this exact ref.
    #[must_use]
    pub fn presentation(&self) -> &CapabilityPresentation {
        &self.presentation
    }

    /// Returns the selector declared by this exact registration, when present.
    #[must_use]
    pub fn selector(&self) -> Option<&CapabilitySelector> {
        self.selector.as_ref()
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
    selector_ids: BTreeMap<CapabilitySelector, String>,
    current_selectors: BTreeMap<CapabilitySelector, CapabilityRef>,
}

impl CapabilityRegistry {
    /// Registers a selector-aware capability and marks it current for that selector.
    pub fn register_selector_current(
        &mut self,
        registration: CapabilityRegistration,
    ) -> Result<(), CapabilityRegistryError> {
        let reference = registration.reference().clone();
        let selector = registration.selector().cloned().ok_or_else(|| {
            CapabilityRegistryError::MissingSelector { reference: reference.clone() }
        })?;
        self.register(registration)?;
        self.current_selectors.insert(selector, reference);
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
        let selector = registration.selector().cloned();
        if let Some(selector) = &selector {
            self.validate_selector_binding(selector, &reference.id)?;
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
        self.registrations.insert(reference.clone(), registration);
        if let Some(selector) = selector {
            self.selector_ids.insert(selector, reference.id);
        }
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

    /// Returns the current exact ref selected by one Workflow modality and mode.
    #[must_use]
    pub fn current_for_selector(&self, selector: &CapabilitySelector) -> Option<CapabilityRef> {
        self.current_selectors.get(selector).cloned()
    }

    /// Returns whether any selector is registered under an output modality.
    #[must_use]
    pub fn contains_selector_type(&self, type_id: &str) -> bool {
        self.selector_ids.keys().any(|selector| selector.type_id == type_id)
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

    /// Returns the current exact refs used for new-node discovery.
    #[must_use]
    pub fn current_references(&self) -> Vec<CapabilityRef> {
        let mut references = self.current_selectors.values().cloned().collect::<Vec<_>>();
        references.sort_unstable();
        references.dedup();
        references
    }

    fn validate_selector_binding(
        &self,
        selector: &CapabilitySelector,
        attempted_id: &str,
    ) -> Result<(), CapabilityRegistryError> {
        if selector.type_id.is_empty() || selector.mode.is_empty() {
            return Err(CapabilityRegistryError::EmptySelector { selector: selector.clone() });
        }
        if let Some(registered_id) = self.selector_ids.get(selector)
            && registered_id != attempted_id
        {
            return Err(CapabilityRegistryError::SelectorRebind {
                selector: selector.clone(),
                registered_id: registered_id.clone(),
                attempted_id: attempted_id.to_owned(),
            });
        }
        Ok(())
    }
}
