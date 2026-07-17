//! Generation Task identities, coordinates, target, time, and revision.

use std::fmt;

use engine::node_capability::{NodeCapabilityContractRef, WorkflowNodeExecutionId, WorkflowRunId};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::GenerationProfileRef;
use projects::project::domain::ProjectId;
use uuid::{Uuid, Variant, Version};

use super::GenerationTaskDomainError;

const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
const MAX_PROVIDER_ID_BYTES: usize = 128;
const MAX_PROVIDER_HANDLE_BYTES: usize = 512;

/// RFC 9562 UUIDv4 identity of one local Generation Task.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationTaskId(Uuid);

impl GenerationTaskId {
    /// Restores an identity only from an RFC 9562 UUIDv4.
    pub fn from_uuid(value: Uuid) -> Result<Self, GenerationTaskDomainError> {
        if value.get_version() != Some(Version::Random) || value.get_variant() != Variant::RFC4122 {
            return Err(GenerationTaskDomainError::InvalidIdentity);
        }
        Ok(Self(value))
    }

    /// Returns the UUID without selecting a boundary encoding.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for GenerationTaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.hyphenated())
    }
}

/// Bounded caller-owned idempotency key scoped to one Project.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskIdempotencyKey(String);

impl GenerationTaskIdempotencyKey {
    /// Validates a non-empty opaque key.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationTaskDomainError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_IDEMPOTENCY_KEY_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(GenerationTaskDomainError::InvalidIdempotencyKey);
        }
        Ok(Self(value))
    }

    /// Returns the exact key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! provider_identity {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Validates the frozen lower-case dot-and-hyphen identity grammar.
            pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationTaskDomainError> {
                let value = value.into();
                if !valid_provider_identity(&value) {
                    return Err(GenerationTaskDomainError::InvalidProviderIdentity);
                }
                Ok(Self(value))
            }

            /// Returns the canonical identity text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

provider_identity!(GenerationProviderId, "Stable identity of one Generation Provider.");
provider_identity!(GenerationProviderRouteId, "Stable identity of one exact provider route.");

fn valid_provider_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_ID_BYTES
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            matches!(bytes.next(), Some(b'a'..=b'z'))
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// Opaque provider-scoped identity of accepted remote work.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationProviderTaskHandle(String);

impl GenerationProviderTaskHandle {
    /// Validates a bounded opaque handle without interpreting its contents.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationTaskDomainError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_PROVIDER_HANDLE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(GenerationTaskDomainError::InvalidProviderTaskHandle);
        }
        Ok(Self(value))
    }

    /// Returns the exact opaque value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact Project, Workflow, Run, node, and node-execution coordinates.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskOrigin {
    project_id: ProjectId,
    workflow_id: WorkflowId,
    workflow_revision: WorkflowRevision,
    workflow_run_id: WorkflowRunId,
    workflow_node_id: WorkflowNodeId,
    workflow_node_execution_id: WorkflowNodeExecutionId,
    capability_contract_ref: NodeCapabilityContractRef,
}

impl GenerationTaskOrigin {
    /// Combines already-validated authoritative identities.
    #[must_use]
    pub const fn new(
        project_id: ProjectId,
        workflow_id: WorkflowId,
        workflow_revision: WorkflowRevision,
        workflow_run_id: WorkflowRunId,
        workflow_node_id: WorkflowNodeId,
        workflow_node_execution_id: WorkflowNodeExecutionId,
        capability_contract_ref: NodeCapabilityContractRef,
    ) -> Self {
        Self {
            project_id,
            workflow_id,
            workflow_revision,
            workflow_run_id,
            workflow_node_id,
            workflow_node_execution_id,
            capability_contract_ref,
        }
    }

    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Returns the exact Workflow.
    #[must_use]
    pub const fn workflow_id(&self) -> WorkflowId {
        self.workflow_id
    }

    /// Returns the frozen Workflow revision.
    #[must_use]
    pub const fn workflow_revision(&self) -> WorkflowRevision {
        self.workflow_revision
    }

    /// Returns the exact Workflow Run.
    #[must_use]
    pub const fn workflow_run_id(&self) -> WorkflowRunId {
        self.workflow_run_id
    }

    /// Returns the exact Workflow node.
    #[must_use]
    pub const fn workflow_node_id(&self) -> WorkflowNodeId {
        self.workflow_node_id
    }

    /// Returns the exact planned node execution.
    #[must_use]
    pub const fn workflow_node_execution_id(&self) -> WorkflowNodeExecutionId {
        self.workflow_node_execution_id
    }

    /// Returns the exact Node Capability contract used by the frozen plan.
    #[must_use]
    pub const fn capability_contract_ref(&self) -> &NodeCapabilityContractRef {
        &self.capability_contract_ref
    }
}

/// Immutable admitted profile, provider, and route selection.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskTarget {
    generation_profile_ref: GenerationProfileRef,
    provider_id: GenerationProviderId,
    route_id: GenerationProviderRouteId,
}

impl GenerationTaskTarget {
    /// Combines an exact Generation Profile with one provider route.
    #[must_use]
    pub const fn new(
        generation_profile_ref: GenerationProfileRef,
        provider_id: GenerationProviderId,
        route_id: GenerationProviderRouteId,
    ) -> Self {
        Self { generation_profile_ref, provider_id, route_id }
    }

    /// Returns the provider-independent profile selection.
    #[must_use]
    pub const fn generation_profile_ref(&self) -> &GenerationProfileRef {
        &self.generation_profile_ref
    }

    /// Returns the exact provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &GenerationProviderId {
        &self.provider_id
    }

    /// Returns the exact provider route identity.
    #[must_use]
    pub const fn route_id(&self) -> &GenerationProviderRouteId {
        &self.route_id
    }
}

/// Non-negative UTC milliseconds used by Generation Task transitions.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationTaskTimestamp(i64);

impl GenerationTaskTimestamp {
    /// Restores a non-negative UTC-millisecond timestamp.
    pub const fn from_utc_milliseconds(value: i64) -> Result<Self, GenerationTaskDomainError> {
        if value < 0 { Err(GenerationTaskDomainError::InvalidTimestamp) } else { Ok(Self(value)) }
    }

    /// Returns UTC milliseconds.
    #[must_use]
    pub const fn as_utc_milliseconds(self) -> i64 {
        self.0
    }
}

/// Non-zero optimistic-lock revision of one Generation Task.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationTaskRevision(u64);

impl GenerationTaskRevision {
    /// Returns the creation revision.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Restores a non-zero revision.
    pub const fn try_new(value: u64) -> Result<Self, GenerationTaskDomainError> {
        if value == 0 { Err(GenerationTaskDomainError::InvalidRevision) } else { Ok(Self(value)) }
    }

    /// Returns the stored revision number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn next(self) -> Result<Self, GenerationTaskDomainError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(GenerationTaskDomainError::InvalidRevision),
        }
    }
}

/// Canonical SHA-256 digest of immutable task admission facts.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskRequestHash([u8; 32]);

impl GenerationTaskRequestHash {
    /// Restores exact SHA-256 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns exact SHA-256 bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}
