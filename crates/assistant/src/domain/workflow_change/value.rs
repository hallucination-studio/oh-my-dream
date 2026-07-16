use std::collections::BTreeSet;

use projects::project::domain::ProjectId;
use uuid::{Uuid, Variant, Version};

use super::super::{
    AssistantApprovalScopeId, AssistantModelInvocationId, AssistantRepairActivationId,
    AssistantSessionId, AssistantWorkflowChangeId,
};

const MAX_CARRIER_BYTES: usize = 1024 * 1024;

/// Workflow Change invariant or transition failure.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssistantWorkflowChangeError {
    #[error("Assistant Workflow Change value is invalid")]
    InvalidValue,
    #[error("Assistant Workflow Change candidate exceeds its bound")]
    CandidateTooLarge,
    #[error("Assistant Workflow Change transition is invalid")]
    InvalidTransition,
    #[error("Assistant review evidence is invalid")]
    ReviewEvidenceInvalid,
    #[error("Assistant approval has expired")]
    ApprovalExpired,
}

macro_rules! opaque_ascii {
    ($name:ident) => {
        #[derive(Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AssistantWorkflowChangeError> {
                let value = value.into();
                if value.is_empty() || value.len() > 256 || !value.is_ascii() {
                    Err(AssistantWorkflowChangeError::InvalidValue)
                } else {
                    Ok(Self(value))
                }
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

opaque_ascii!(AssistantModelContinuationRef);
opaque_ascii!(AssistantModelIdentity);
opaque_ascii!(AssistantToolCallId);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowRevisionBoundaryValue(u64);

impl WorkflowRevisionBoundaryValue {
    pub fn new(value: u64) -> Result<Self, AssistantWorkflowChangeError> {
        if value == 0 { Err(AssistantWorkflowChangeError::InvalidValue) } else { Ok(Self(value)) }
    }
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssistantWorkflowMutation(Vec<u8>);

impl AssistantWorkflowMutation {
    pub fn new(value: Vec<u8>) -> Result<Self, AssistantWorkflowChangeError> {
        validate_carrier(&value).map(|()| Self(value))
    }
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssistantWorkflowReadinessIssueBoundaryValue(Vec<u8>);

impl AssistantWorkflowReadinessIssueBoundaryValue {
    pub fn new(value: Vec<u8>) -> Result<Self, AssistantWorkflowChangeError> {
        validate_carrier(&value).map(|()| Self(value))
    }
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0
    }
}

macro_rules! digest_value {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        pub struct $name([u8; 32]);
        impl $name {
            #[must_use]
            pub const fn new(value: [u8; 32]) -> Self {
                Self(value)
            }
            #[must_use]
            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

digest_value!(AssistantWorkflowMutationDigest);
digest_value!(AssistantWorkflowFingerprint);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssistantWorkflowStableAliasEntry {
    alias: String,
    resulting_node_id: [u8; 16],
}

impl AssistantWorkflowStableAliasEntry {
    pub fn new(
        alias: impl Into<String>,
        resulting_node_id: [u8; 16],
    ) -> Result<Self, AssistantWorkflowChangeError> {
        let alias = alias.into();
        let node_id = Uuid::from_bytes(resulting_node_id);
        if !valid_alias(&alias)
            || node_id.get_version() != Some(Version::Random)
            || node_id.get_variant() != Variant::RFC4122
        {
            return Err(AssistantWorkflowChangeError::InvalidValue);
        }
        Ok(Self { alias, resulting_node_id })
    }
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
    #[must_use]
    pub const fn resulting_node_id(&self) -> [u8; 16] {
        self.resulting_node_id
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct AssistantWorkflowStableAliasSet(Vec<AssistantWorkflowStableAliasEntry>);

impl AssistantWorkflowStableAliasSet {
    pub fn new(
        mut entries: Vec<AssistantWorkflowStableAliasEntry>,
    ) -> Result<Self, AssistantWorkflowChangeError> {
        if entries.len() > 128 {
            return Err(AssistantWorkflowChangeError::CandidateTooLarge);
        }
        entries.sort_by(|left, right| left.alias.cmp(&right.alias));
        let unique = entries.iter().map(|entry| &entry.alias).collect::<BTreeSet<_>>();
        if unique.len() != entries.len() {
            return Err(AssistantWorkflowChangeError::InvalidValue);
        }
        Ok(Self(entries))
    }
    #[must_use]
    pub fn entries(&self) -> &[AssistantWorkflowStableAliasEntry] {
        &self.0
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssistantUserIntent(String);

impl AssistantUserIntent {
    pub fn new(value: impl AsRef<str>) -> Result<Self, AssistantWorkflowChangeError> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.len() > 16 * 1024 {
            Err(AssistantWorkflowChangeError::InvalidValue)
        } else {
            Ok(Self(value.to_owned()))
        }
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantWorkflowChangeLineage {
    UserMessage { invocation_id: AssistantModelInvocationId, intent: AssistantUserIntent },
    ReviewedRepair { activation_id: AssistantRepairActivationId, failed_workflow_run_id: [u8; 16] },
}

macro_rules! non_negative_time {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i64);
        impl $name {
            pub fn new(value: i64) -> Result<Self, AssistantWorkflowChangeError> {
                if value < 0 {
                    Err(AssistantWorkflowChangeError::InvalidValue)
                } else {
                    Ok(Self(value))
                }
            }
            #[must_use]
            pub const fn epoch_ms(self) -> i64 {
                self.0
            }
        }
    };
}

non_negative_time!(AssistantReviewedAt);
non_negative_time!(AssistantWorkflowChangeExpiry);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssistantContractEpoch(u32);

impl AssistantContractEpoch {
    pub fn new(value: u32) -> Result<Self, AssistantWorkflowChangeError> {
        if value == 0 { Err(AssistantWorkflowChangeError::InvalidValue) } else { Ok(Self(value)) }
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AssistantReviewVerdict {
    Pass,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantReviewReceipt {
    pub change_id: AssistantWorkflowChangeId,
    pub mutation_digest: AssistantWorkflowMutationDigest,
    pub reviewer_contract_epoch: AssistantContractEpoch,
    pub reviewer_model: AssistantModelIdentity,
    pub reviewer_invocation_id: AssistantModelInvocationId,
    pub reviewer_tool_call_id: AssistantToolCallId,
    pub verdict: AssistantReviewVerdict,
    pub reviewed_at: AssistantReviewedAt,
}

impl AssistantReviewReceipt {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        change_id: AssistantWorkflowChangeId,
        mutation_digest: AssistantWorkflowMutationDigest,
        reviewer_contract_epoch: AssistantContractEpoch,
        reviewer_model: AssistantModelIdentity,
        reviewer_invocation_id: AssistantModelInvocationId,
        reviewer_tool_call_id: AssistantToolCallId,
        verdict: AssistantReviewVerdict,
        reviewed_at: AssistantReviewedAt,
    ) -> Self {
        Self {
            change_id,
            mutation_digest,
            reviewer_contract_epoch,
            reviewer_model,
            reviewer_invocation_id,
            reviewer_tool_call_id,
            verdict,
            reviewed_at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantWorkflowChangeState {
    Proposed,
    ReviewRejected,
    AwaitingApproval,
    Rejected,
    Applying,
    Applied,
    ApplyFailed,
    Expired,
}

/// Exact immutable scope echoed by one human approval decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowChangeDecisionScope {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub change_id: AssistantWorkflowChangeId,
    pub approval_scope_id: AssistantApprovalScopeId,
    pub mutation_digest: AssistantWorkflowMutationDigest,
}

fn validate_carrier(value: &[u8]) -> Result<(), AssistantWorkflowChangeError> {
    if value.is_empty() || value.len() > MAX_CARRIER_BYTES {
        Err(AssistantWorkflowChangeError::InvalidValue)
    } else {
        Ok(())
    }
}

fn valid_alias(value: &str) -> bool {
    let mut bytes = value.bytes();
    (1..=64).contains(&value.len())
        && matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
