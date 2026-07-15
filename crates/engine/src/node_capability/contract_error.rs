//! Closed static contract and parameter-set failures.

use thiserror::Error;

use super::NodeCapabilityParameterKey;

/// Invalid static capability contract definition.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NodeCapabilityContractError {
    /// Contract ID did not match its canonical grammar.
    #[error("node capability contract ID is invalid")]
    InvalidContractId,
    /// Major version was zero.
    #[error("node capability contract version is invalid")]
    InvalidContractVersion,
    /// A parameter, input, output, role, or choice key was invalid.
    #[error("node capability key is invalid")]
    InvalidKey,
    /// A declared bound or allowed-value set was invalid.
    #[error("node capability constraint is invalid")]
    InvalidConstraint,
    /// A default did not satisfy its declared constraint.
    #[error("node capability parameter default is invalid")]
    InvalidDefault,
    /// Two declarations used the same key.
    #[error("node capability contract contains a duplicate key")]
    DuplicateKey,
    /// The contract did not contain exactly one primary output.
    #[error("node capability contract must contain exactly one primary output")]
    InvalidPrimaryOutputCount,
    /// An input or output collection exceeded its frozen bound.
    #[error("node capability contract collection size is invalid")]
    InvalidCollectionSize,
}

/// Closed parameter-normalization failure category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityParameterErrorCategory {
    /// A supplied key was not declared.
    UnknownParameter,
    /// A required value was absent.
    RequiredParameterMissing,
    /// A value variant differed from its constraint.
    ParameterValueKindMismatch,
    /// A value was outside declared bounds.
    ParameterValueOutOfBounds,
    /// A choice was not declared.
    ParameterChoiceNotDeclared,
    /// More than 64 parameters were supplied.
    ParameterSetTooLarge,
}

/// Precise target of one parameter failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityParameterErrorTarget {
    /// The parameter set as a whole.
    ParameterSet,
    /// One exact parameter.
    Parameter(NodeCapabilityParameterKey),
}

/// Structured failure returned while normalizing parameters.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("node capability parameter normalization failed")]
pub struct NodeCapabilityParameterError {
    category: NodeCapabilityParameterErrorCategory,
    target: NodeCapabilityParameterErrorTarget,
}

impl NodeCapabilityParameterError {
    pub(crate) const fn new(
        category: NodeCapabilityParameterErrorCategory,
        target: NodeCapabilityParameterErrorTarget,
    ) -> Self {
        Self { category, target }
    }

    /// Returns the closed parameter failure category.
    #[must_use]
    pub const fn category(&self) -> NodeCapabilityParameterErrorCategory {
        self.category
    }

    /// Returns the precise parameter failure target.
    #[must_use]
    pub const fn target(&self) -> &NodeCapabilityParameterErrorTarget {
        &self.target
    }
}
