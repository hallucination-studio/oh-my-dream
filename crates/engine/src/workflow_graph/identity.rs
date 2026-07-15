//! Workflow-local identities and monotonic revision.

use uuid::{Uuid, Variant, Version};

use super::WorkflowGraphConstructionError;

macro_rules! workflow_graph_uuid {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Restores the identity only from an RFC 9562 UUIDv4.
            pub fn from_uuid(value: Uuid) -> Result<Self, WorkflowGraphConstructionError> {
                if value.get_version() != Some(Version::Random)
                    || value.get_variant() != Variant::RFC4122
                {
                    return Err(WorkflowGraphConstructionError::IdentityNotVersionFour);
                }
                Ok(Self(value))
            }

            /// Returns the UUID without selecting a wire representation.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

workflow_graph_uuid!(WorkflowId, "Identity of one Workflow aggregate.");
workflow_graph_uuid!(WorkflowNodeId, "Workflow-local identity of one node.");
workflow_graph_uuid!(
    WorkflowMutationRequestId,
    "Idempotency identity of one Workflow mutation request."
);

/// Non-zero aggregate revision; creation starts at one.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowRevision(u64);

impl WorkflowRevision {
    /// Restores a non-zero revision.
    pub const fn new(value: u64) -> Result<Self, WorkflowGraphConstructionError> {
        if value == 0 { Err(WorkflowGraphConstructionError::RevisionZero) } else { Ok(Self(value)) }
    }

    /// Returns the stored revision number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
