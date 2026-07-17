//! Typed values for the three-kind Desktop effect outbox.

use assets::asset::application::AssetFinalizeContentEffect;
use assistant::application::AssistantApplyWorkflowChangeEffect;
use engine::workflow::WorkflowExecuteRunEffect;
use uuid::{Uuid, Variant, Version};

/// Invalid Desktop post-commit effect value.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopPostCommitEffectValueError {
    /// An identity is not an RFC 9562 UUIDv4.
    #[error("Desktop post-commit effect identity must be an RFC 9562 UUIDv4")]
    InvalidIdentity,
    /// A durable timestamp is negative.
    #[error("Desktop post-commit effect timestamp must be non-negative")]
    InvalidTimestamp,
}

macro_rules! desktop_uuid_identity {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Restores an exact RFC 9562 UUIDv4 identity.
            pub fn from_uuid(value: Uuid) -> Result<Self, DesktopPostCommitEffectValueError> {
                if value.get_version() == Some(Version::Random)
                    && value.get_variant() == Variant::RFC4122
                {
                    Ok(Self(value))
                } else {
                    Err(DesktopPostCommitEffectValueError::InvalidIdentity)
                }
            }

            /// Returns the UUID without choosing a wire encoding.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

desktop_uuid_identity!(DesktopPostCommitEffectId, "Identity of one durable post-commit effect.");
desktop_uuid_identity!(DesktopApplicationInstanceId, "Identity of one Desktop process instance.");

/// Non-negative UTC-millisecond timestamp stored with outbox delivery state.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesktopPostCommitTimestamp(i64);

impl DesktopPostCommitTimestamp {
    /// Restores a non-negative UTC-millisecond timestamp.
    pub const fn from_epoch_millis(value: i64) -> Result<Self, DesktopPostCommitEffectValueError> {
        if value < 0 {
            Err(DesktopPostCommitEffectValueError::InvalidTimestamp)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the stored UTC milliseconds.
    #[must_use]
    pub const fn epoch_millis(self) -> i64 {
        self.0
    }
}

/// Closed union of committed business-owned effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopPostCommitEffect {
    /// Execute one already-admitted Workflow Run.
    Workflow(WorkflowExecuteRunEffect),
    /// Finalize one already-committed Asset content object.
    Asset(AssetFinalizeContentEffect),
    /// Apply one already-approved Assistant Workflow change.
    Assistant(AssistantApplyWorkflowChangeEffect),
}

/// Durable delivery state of one post-commit effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopPostCommitEffectState {
    /// Available for the normal worker to claim.
    Ready,
    /// Exclusively claimed by one current or prior Desktop instance.
    Claimed {
        /// Claiming process identity.
        instance_id: DesktopApplicationInstanceId,
        /// Exact claim time.
        claimed_at: DesktopPostCommitTimestamp,
    },
    /// Consumer reached its durable success outcome.
    Completed {
        /// Exact completion time.
        completed_at: DesktopPostCommitTimestamp,
    },
    /// Recovery proved the effect must never execute.
    Abandoned {
        /// Exact abandonment time.
        abandoned_at: DesktopPostCommitTimestamp,
        /// Closed recovery reason.
        reason: DesktopPostCommitEffectAbandonReason,
    },
}

/// Closed reason for abandoning a Workflow effect during recovery.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum DesktopPostCommitEffectAbandonReason {
    /// The owning Run was failed conservatively after process restart.
    WorkflowInterruptedByRestart,
    /// The owning business state had already reached another terminal outcome.
    OwningStateAlreadyTerminal,
}
