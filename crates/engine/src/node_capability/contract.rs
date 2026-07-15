//! Static capability contracts and generic parameter normalization.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    NodeCapabilityContractError, NodeCapabilityContractRef, NodeCapabilityInputKey,
    NodeCapabilityInputRoleKey, NodeCapabilityNormalizedParameters, NodeCapabilityOutputKey,
    NodeCapabilityParameterConstraint, NodeCapabilityParameterError,
    NodeCapabilityParameterErrorCategory, NodeCapabilityParameterErrorTarget,
    NodeCapabilityParameterKey, NodeCapabilityParameterSet, NodeCapabilityParameterValue,
    WorkflowDataType,
};

/// Whether a declared parameter is required or defaulted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityParameterPresence {
    /// Caller must supply the value.
    Required,
    /// Normalization inserts this value when absent.
    OptionalWithDefault(NodeCapabilityParameterValue),
}

/// Complete declaration of one node parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityParameterContract {
    key: NodeCapabilityParameterKey,
    constraint: NodeCapabilityParameterConstraint,
    presence: NodeCapabilityParameterPresence,
}

impl NodeCapabilityParameterContract {
    /// Declares a required parameter.
    #[must_use]
    pub const fn required(
        key: NodeCapabilityParameterKey,
        constraint: NodeCapabilityParameterConstraint,
    ) -> Self {
        Self { key, constraint, presence: NodeCapabilityParameterPresence::Required }
    }

    /// Declares a defaulted parameter after validating its default.
    pub fn optional_with_default(
        key: NodeCapabilityParameterKey,
        constraint: NodeCapabilityParameterConstraint,
        default: NodeCapabilityParameterValue,
    ) -> Result<Self, NodeCapabilityContractError> {
        constraint.validate_constraint_definition()?;
        constraint
            .validate_parameter_value(&default)
            .map_err(|_| NodeCapabilityContractError::InvalidDefault)?;
        Ok(Self {
            key,
            constraint,
            presence: NodeCapabilityParameterPresence::OptionalWithDefault(default),
        })
    }

    /// Returns the parameter key.
    #[must_use]
    pub const fn key(&self) -> &NodeCapabilityParameterKey {
        &self.key
    }

    /// Returns the closed value constraint.
    #[must_use]
    pub const fn constraint(&self) -> &NodeCapabilityParameterConstraint {
        &self.constraint
    }

    /// Returns whether the parameter is required or defaulted.
    #[must_use]
    pub const fn presence(&self) -> &NodeCapabilityParameterPresence {
        &self.presence
    }
}

/// Non-empty accepted runtime types for one ordered-input role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowAcceptedDataTypeSet(BTreeSet<WorkflowDataType>);

impl WorkflowAcceptedDataTypeSet {
    /// Creates a set that excludes Text and is not empty.
    pub fn try_new(
        values: impl IntoIterator<Item = WorkflowDataType>,
    ) -> Result<Self, NodeCapabilityContractError> {
        let values = values.into_iter().collect::<BTreeSet<_>>();
        if values.is_empty() || values.contains(&WorkflowDataType::Text) {
            return Err(NodeCapabilityContractError::InvalidConstraint);
        }
        Ok(Self(values))
    }

    /// Reports whether the runtime type is accepted.
    #[must_use]
    pub fn contains(&self, value: WorkflowDataType) -> bool {
        self.0.contains(&value)
    }
}

/// Exact binding shape accepted by one capability input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityInputBindingContract {
    /// Optional role-free single value.
    OptionalSingleValue {
        /// Exact accepted runtime type.
        data_type: WorkflowDataType,
    },
    /// Required role-free single value.
    RequiredSingleValue {
        /// Exact accepted runtime type.
        data_type: WorkflowDataType,
    },
    /// Ordered role-bearing references.
    OrderedReferences {
        /// Minimum business cardinality.
        minimum_items: u32,
        /// Optional maximum business cardinality.
        maximum_items: Option<u32>,
        /// Accepted concrete media types by declared role.
        accepted_data_types_by_role:
            BTreeMap<NodeCapabilityInputRoleKey, WorkflowAcceptedDataTypeSet>,
    },
}

impl NodeCapabilityInputBindingContract {
    fn validate(&self) -> Result<(), NodeCapabilityContractError> {
        if let Self::OrderedReferences {
            minimum_items,
            maximum_items,
            accepted_data_types_by_role,
        } = self
        {
            let invalid_maximum = maximum_items.is_some_and(|maximum| maximum < *minimum_items);
            if invalid_maximum || accepted_data_types_by_role.is_empty() {
                return Err(NodeCapabilityContractError::InvalidConstraint);
            }
        }
        Ok(())
    }
}

/// Complete declaration of one named node input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityInputContract {
    key: NodeCapabilityInputKey,
    binding: NodeCapabilityInputBindingContract,
}

impl NodeCapabilityInputContract {
    /// Creates a validated input declaration.
    pub fn new(
        key: NodeCapabilityInputKey,
        binding: NodeCapabilityInputBindingContract,
    ) -> Result<Self, NodeCapabilityContractError> {
        binding.validate()?;
        Ok(Self { key, binding })
    }

    /// Returns the input key.
    #[must_use]
    pub const fn key(&self) -> &NodeCapabilityInputKey {
        &self.key
    }

    /// Returns the binding contract.
    #[must_use]
    pub const fn binding(&self) -> &NodeCapabilityInputBindingContract {
        &self.binding
    }
}

/// Complete declaration of one mandatory node output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityOutputContract {
    key: NodeCapabilityOutputKey,
    data_type: WorkflowDataType,
    is_primary: bool,
}

impl NodeCapabilityOutputContract {
    /// Creates an output declaration.
    #[must_use]
    pub const fn new(
        key: NodeCapabilityOutputKey,
        data_type: WorkflowDataType,
        is_primary: bool,
    ) -> Self {
        Self { key, data_type, is_primary }
    }

    /// Returns the output key.
    #[must_use]
    pub const fn key(&self) -> &NodeCapabilityOutputKey {
        &self.key
    }

    /// Returns the exact output runtime type.
    #[must_use]
    pub const fn data_type(&self) -> WorkflowDataType {
        self.data_type
    }

    /// Reports whether this is the presentation output.
    #[must_use]
    pub const fn is_primary(&self) -> bool {
        self.is_primary
    }
}

/// Closed business classification of capability execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityExecutionKind {
    /// Pure in-memory value production.
    PureValue,
    /// Read of an existing managed Asset.
    ManagedAssetRead,
    /// Provider-backed content generation.
    ContentGeneration,
    /// Media transformation.
    MediaTransformation,
    /// Content analysis.
    ContentAnalysis,
}

/// Immutable versioned structural contract for one exact capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityContract {
    contract_ref: NodeCapabilityContractRef,
    parameters: Vec<NodeCapabilityParameterContract>,
    inputs: Vec<NodeCapabilityInputContract>,
    outputs: Vec<NodeCapabilityOutputContract>,
    execution_kind: NodeCapabilityExecutionKind,
}

impl NodeCapabilityContract {
    /// Builds a contract after validating declaration invariants.
    pub fn try_new(
        contract_ref: NodeCapabilityContractRef,
        parameters: Vec<NodeCapabilityParameterContract>,
        inputs: Vec<NodeCapabilityInputContract>,
        outputs: Vec<NodeCapabilityOutputContract>,
        execution_kind: NodeCapabilityExecutionKind,
    ) -> Result<Self, NodeCapabilityContractError> {
        if parameters.len() > 64 || inputs.len() > 64 || outputs.is_empty() || outputs.len() > 64 {
            return Err(NodeCapabilityContractError::InvalidCollectionSize);
        }
        for parameter in &parameters {
            parameter.constraint.validate_constraint_definition()?;
            if let NodeCapabilityParameterPresence::OptionalWithDefault(default) =
                &parameter.presence
            {
                parameter
                    .constraint
                    .validate_parameter_value(default)
                    .map_err(|_| NodeCapabilityContractError::InvalidDefault)?;
            }
        }
        ensure_unique(parameters.iter().map(NodeCapabilityParameterContract::key))?;
        ensure_unique(inputs.iter().map(NodeCapabilityInputContract::key))?;
        ensure_unique(outputs.iter().map(NodeCapabilityOutputContract::key))?;
        if outputs.iter().filter(|output| output.is_primary()).count() != 1 {
            return Err(NodeCapabilityContractError::InvalidPrimaryOutputCount);
        }
        Ok(Self { contract_ref, parameters, inputs, outputs, execution_kind })
    }

    /// Returns the exact contract identity.
    #[must_use]
    pub const fn contract_ref(&self) -> &NodeCapabilityContractRef {
        &self.contract_ref
    }

    /// Returns parameter declarations in presentation order.
    #[must_use]
    pub fn parameters(&self) -> &[NodeCapabilityParameterContract] {
        &self.parameters
    }

    /// Returns input declarations in presentation order.
    #[must_use]
    pub fn inputs(&self) -> &[NodeCapabilityInputContract] {
        &self.inputs
    }

    /// Returns output declarations in presentation order.
    #[must_use]
    pub fn outputs(&self) -> &[NodeCapabilityOutputContract] {
        &self.outputs
    }

    /// Returns the business execution classification.
    #[must_use]
    pub const fn execution_kind(&self) -> NodeCapabilityExecutionKind {
        self.execution_kind
    }

    /// Validates supplied values and inserts declared defaults.
    pub fn normalize_node_parameters(
        &self,
        supplied: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        for (key, _) in supplied.iter() {
            if !self.parameters.iter().any(|parameter| parameter.key() == key) {
                return Err(parameter_error(
                    NodeCapabilityParameterErrorCategory::UnknownParameter,
                    key.clone(),
                ));
            }
        }
        let mut normalized = BTreeMap::new();
        for parameter in &self.parameters {
            let value = match supplied.get(parameter.key()) {
                Some(value) => value.clone(),
                None => match &parameter.presence {
                    NodeCapabilityParameterPresence::Required => {
                        return Err(parameter_error(
                            NodeCapabilityParameterErrorCategory::RequiredParameterMissing,
                            parameter.key.clone(),
                        ));
                    }
                    NodeCapabilityParameterPresence::OptionalWithDefault(value) => value.clone(),
                },
            };
            parameter
                .constraint
                .validate_parameter_value(&value)
                .map_err(|category| parameter_error(category, parameter.key.clone()))?;
            normalized.insert(parameter.key.clone(), value);
        }
        Ok(NodeCapabilityNormalizedParameters::from_validated(normalized))
    }
}

fn ensure_unique<'a, T: Ord + 'a>(
    mut values: impl Iterator<Item = &'a T>,
) -> Result<(), NodeCapabilityContractError> {
    let mut seen = BTreeSet::new();
    if values.all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(NodeCapabilityContractError::DuplicateKey)
    }
}

fn parameter_error(
    category: NodeCapabilityParameterErrorCategory,
    key: NodeCapabilityParameterKey,
) -> NodeCapabilityParameterError {
    NodeCapabilityParameterError::new(category, NodeCapabilityParameterErrorTarget::Parameter(key))
}
