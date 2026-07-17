use super::*;

impl ChangeRow {
    pub(super) fn from_domain(change: &AssistantWorkflowChangeAggregate) -> Self {
        Self {
            base_revision: change.base_workflow_revision().get(),
            mutations: change
                .ordered_mutations()
                .iter()
                .map(|value| value.canonical_bytes().to_vec())
                .collect(),
            aliases: change
                .stable_aliases()
                .entries()
                .iter()
                .map(|entry| (entry.alias().to_owned(), entry.resulting_node_id()))
                .collect(),
            readiness: change
                .readiness_issues()
                .iter()
                .map(|value| value.canonical_bytes().to_vec())
                .collect(),
            mutation_digest: change.mutation_digest().as_bytes(),
            fingerprint: change.resulting_workflow_fingerprint().as_bytes(),
            lineage: LineageRow::from_domain(change.lineage()),
            review: change.review().map(ReviewRow::from_domain),
            approval_scope_id: *change.approval_scope_id().as_uuid().as_bytes(),
            continuation_ref: change.continuation_ref().map(|value| value.as_str().to_owned()),
            expires_at: change.expires_at().epoch_ms(),
            applied_receipt: change
                .applied_workflow_receipt()
                .map(|value| value.canonical_bytes().to_vec()),
            admitted_run: change
                .admitted_workflow_run()
                .map(|value| value.canonical_bytes().to_vec()),
            continuation_outcome: encode_continuation_outcome(change.continuation_outcome()),
        }
    }

    pub(super) fn into_domain(
        self,
        change_id: AssistantWorkflowChangeId,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        state: AssistantWorkflowChangeState,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let digest = AssistantWorkflowMutationDigest::new(self.mutation_digest);
        let candidate = AssistantWorkflowChangeCandidate {
            id: change_id,
            project_id,
            session_id,
            base_workflow_revision: WorkflowRevisionBoundaryValue::new(self.base_revision)
                .map_err(|_| corrupt())?,
            ordered_mutations: self
                .mutations
                .into_iter()
                .map(AssistantWorkflowMutation::new)
                .collect::<Result<_, _>>()
                .map_err(|_| corrupt())?,
            stable_aliases: AssistantWorkflowStableAliasSet::new(
                self.aliases
                    .into_iter()
                    .map(|(alias, node)| AssistantWorkflowStableAliasEntry::new(alias, node))
                    .collect::<Result<_, _>>()
                    .map_err(|_| corrupt())?,
            )
            .map_err(|_| corrupt())?,
            readiness_issues: self
                .readiness
                .into_iter()
                .map(AssistantWorkflowReadinessIssueBoundaryValue::new)
                .collect::<Result<_, _>>()
                .map_err(|_| corrupt())?,
            mutation_digest: digest,
            resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new(self.fingerprint),
            lineage: self.lineage.into_domain()?,
            approval_scope_id: AssistantApprovalScopeId::from_uuid(Uuid::from_bytes(
                self.approval_scope_id,
            ))
            .map_err(|_| corrupt())?,
            expires_at: AssistantWorkflowChangeExpiry::new(self.expires_at)
                .map_err(|_| corrupt())?,
        };
        let review = self.review.map(|value| value.into_domain(change_id, digest)).transpose()?;
        AssistantWorkflowChangeAggregate::try_restore(
            candidate,
            review,
            self.continuation_ref
                .map(AssistantModelContinuationRef::new)
                .transpose()
                .map_err(|_| corrupt())?,
            state,
            self.applied_receipt
                .map(AssistantWorkflowApplyReceiptBoundaryValue::new)
                .transpose()
                .map_err(|_| corrupt())?,
            self.admitted_run
                .map(AssistantWorkflowRunBoundaryValue::new)
                .transpose()
                .map_err(|_| corrupt())?,
            decode_continuation_outcome(self.continuation_outcome)?,
        )
        .map_err(|_| corrupt())
    }
}

impl LineageRow {
    fn from_domain(value: &AssistantWorkflowChangeLineage) -> Self {
        match value {
            AssistantWorkflowChangeLineage::UserMessage { invocation_id, intent } => {
                Self::UserMessage {
                    invocation_id: *invocation_id.as_uuid().as_bytes(),
                    intent: intent.as_str().to_owned(),
                }
            }
            AssistantWorkflowChangeLineage::ReviewedRepair {
                activation_id,
                failed_workflow_run_id,
            } => Self::ReviewedRepair {
                activation_id: *activation_id.as_uuid().as_bytes(),
                failed_run_id: *failed_workflow_run_id,
            },
        }
    }

    fn into_domain(self) -> Result<AssistantWorkflowChangeLineage, AssistantApplicationError> {
        match self {
            Self::UserMessage { invocation_id, intent } => {
                Ok(AssistantWorkflowChangeLineage::UserMessage {
                    invocation_id: AssistantModelInvocationId::from_uuid(Uuid::from_bytes(
                        invocation_id,
                    ))
                    .map_err(|_| corrupt())?,
                    intent: AssistantUserIntent::new(intent).map_err(|_| corrupt())?,
                })
            }
            Self::ReviewedRepair { activation_id, failed_run_id } => {
                Ok(AssistantWorkflowChangeLineage::ReviewedRepair {
                    activation_id: AssistantRepairActivationId::from_uuid(Uuid::from_bytes(
                        activation_id,
                    ))
                    .map_err(|_| corrupt())?,
                    failed_workflow_run_id: failed_run_id,
                })
            }
        }
    }
}

impl ReviewRow {
    fn from_domain(value: &AssistantReviewReceipt) -> Self {
        Self {
            contract_epoch: value.reviewer_contract_epoch.get(),
            model: value.reviewer_model.as_str().to_owned(),
            invocation_id: *value.reviewer_invocation_id.as_uuid().as_bytes(),
            tool_call_id: value.reviewer_tool_call_id.as_str().to_owned(),
            verdict: match value.verdict {
                AssistantReviewVerdict::Pass => 1,
                AssistantReviewVerdict::Reject => 2,
            },
            reviewed_at: value.reviewed_at.epoch_ms(),
        }
    }

    fn into_domain(
        self,
        change_id: AssistantWorkflowChangeId,
        digest: AssistantWorkflowMutationDigest,
    ) -> Result<AssistantReviewReceipt, AssistantApplicationError> {
        Ok(AssistantReviewReceipt::new(
            change_id,
            digest,
            AssistantContractEpoch::new(self.contract_epoch).map_err(|_| corrupt())?,
            AssistantModelIdentity::new(self.model).map_err(|_| corrupt())?,
            AssistantModelInvocationId::from_uuid(Uuid::from_bytes(self.invocation_id))
                .map_err(|_| corrupt())?,
            AssistantToolCallId::new(self.tool_call_id).map_err(|_| corrupt())?,
            match self.verdict {
                1 => AssistantReviewVerdict::Pass,
                2 => AssistantReviewVerdict::Reject,
                _ => return Err(corrupt()),
            },
            AssistantReviewedAt::new(self.reviewed_at).map_err(|_| corrupt())?,
        ))
    }
}

fn encode_continuation_outcome(value: AssistantContinuationOutcome) -> u8 {
    match value {
        AssistantContinuationOutcome::Pending => 0,
        AssistantContinuationOutcome::Resumed => 1,
        AssistantContinuationOutcome::Interrupted => 2,
    }
}

fn decode_continuation_outcome(
    value: u8,
) -> Result<AssistantContinuationOutcome, AssistantApplicationError> {
    match value {
        0 => Ok(AssistantContinuationOutcome::Pending),
        1 => Ok(AssistantContinuationOutcome::Resumed),
        2 => Ok(AssistantContinuationOutcome::Interrupted),
        _ => Err(corrupt()),
    }
}
