//! Assistant-owned stable identities.

use uuid::{Uuid, Variant, Version};

/// Invalid Assistant UUID identity.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Assistant identity must be an RFC 9562 UUIDv4")]
pub struct AssistantIdentityError;

macro_rules! assistant_uuid_identity {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Restores an exact RFC 9562 UUIDv4 identity.
            pub fn from_uuid(value: Uuid) -> Result<Self, AssistantIdentityError> {
                if value.get_version() == Some(Version::Random)
                    && value.get_variant() == Variant::RFC4122
                {
                    Ok(Self(value))
                } else {
                    Err(AssistantIdentityError)
                }
            }

            /// Returns the UUID without choosing a boundary encoding.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

assistant_uuid_identity!(AssistantProductionPlanId, "Identity of one Assistant production plan.");
assistant_uuid_identity!(AssistantSessionId, "Identity of one Project-scoped Assistant session.");
assistant_uuid_identity!(AssistantWorkflowChangeId, "Identity of one immutable Workflow change.");
assistant_uuid_identity!(AssistantApprovalScopeId, "Identity of one human approval scope.");
assistant_uuid_identity!(AssistantModelInvocationId, "Identity of one bounded model invocation.");
assistant_uuid_identity!(AssistantRepairActivationId, "Identity of one factual repair activation.");
