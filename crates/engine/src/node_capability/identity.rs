//! Capability identities and behavior-revealing keys.

use std::fmt;

use uuid::{Uuid, Variant, Version};

use super::NodeCapabilityContractError;

fn is_valid_key(value: &str, maximum_bytes: usize) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && !value.is_empty()
        && value.len() <= maximum_bytes
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

macro_rules! capability_key {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            #[doc = "Creates a validated key."]
            pub fn new(value: impl Into<String>) -> Result<Self, NodeCapabilityContractError> {
                let value = value.into();
                if !is_valid_key(&value, 64) {
                    return Err(NodeCapabilityContractError::InvalidKey);
                }
                Ok(Self(value))
            }

            #[doc = "Returns the canonical key text."]
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

capability_key!(NodeCapabilityParameterKey, "Identity of one declared node parameter.");
capability_key!(NodeCapabilityInputKey, "Identity of one declared node input.");
capability_key!(NodeCapabilityOutputKey, "Identity of one declared node output.");
capability_key!(NodeCapabilityInputRoleKey, "Capability-owned role of one ordered input.");
capability_key!(NodeCapabilityChoiceKey, "Capability-owned parameter choice identity.");

/// Stable identity of one versioned capability contract family.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityContractId(String);

impl NodeCapabilityContractId {
    /// Creates an ID with two or more valid dot-separated segments.
    pub fn new(value: impl Into<String>) -> Result<Self, NodeCapabilityContractError> {
        let value = value.into();
        let valid = (3..=128).contains(&value.len())
            && value.split('.').count() >= 2
            && value.split('.').all(|segment| is_valid_key(segment, 128));
        if !valid {
            return Err(NodeCapabilityContractError::InvalidContractId);
        }
        Ok(Self(value))
    }

    /// Returns the canonical ID text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Major and minor version of one capability contract.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityContractVersion {
    major: u16,
    minor: u16,
}

impl NodeCapabilityContractVersion {
    /// Creates a version with a non-zero major component.
    pub const fn new(major: u16, minor: u16) -> Result<Self, NodeCapabilityContractError> {
        if major == 0 {
            Err(NodeCapabilityContractError::InvalidContractVersion)
        } else {
            Ok(Self { major, minor })
        }
    }

    /// Returns the major component.
    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    /// Returns the minor component.
    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

/// Exact immutable identity of one capability contract version.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityContractRef {
    id: NodeCapabilityContractId,
    version: NodeCapabilityContractVersion,
}

impl NodeCapabilityContractRef {
    /// Combines a validated contract ID and version.
    #[must_use]
    pub const fn new(id: NodeCapabilityContractId, version: NodeCapabilityContractVersion) -> Self {
        Self { id, version }
    }

    /// Returns the contract family ID.
    #[must_use]
    pub const fn id(&self) -> &NodeCapabilityContractId {
        &self.id
    }

    /// Returns the exact contract version.
    #[must_use]
    pub const fn version(&self) -> NodeCapabilityContractVersion {
        self.version
    }
}

impl fmt::Display for NodeCapabilityContractRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}.{}", self.id.as_str(), self.version.major, self.version.minor)
    }
}

macro_rules! workflow_uuid {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            #[doc = "Restores an identity only from an RFC 9562 UUIDv4."]
            #[must_use]
            pub fn from_uuid(value: Uuid) -> Option<Self> {
                (value.get_version() == Some(Version::Random)
                    && value.get_variant() == Variant::RFC4122)
                    .then_some(Self(value))
            }

            #[doc = "Returns the UUID without choosing a wire encoding."]
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

workflow_uuid!(WorkflowRunId, "Identity of one admitted Workflow Run.");
workflow_uuid!(WorkflowNodeExecutionId, "Identity of one planned node execution.");
workflow_uuid!(WorkflowInputItemId, "Stable identity of one runtime input item.");
