use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::super::*;

fn id<T>(constructor: impl FnOnce(Uuid) -> Result<T, AssistantIdentityError>) -> T {
    constructor(Uuid::new_v4()).unwrap()
}

fn proposed() -> AssistantWorkflowChangeAggregate {
    AssistantWorkflowChangeAggregate::new(AssistantWorkflowChangeCandidate {
        id: id(AssistantWorkflowChangeId::from_uuid),
        project_id: ProjectId::from_uuid(Uuid::new_v4()).unwrap(),
        session_id: id(AssistantSessionId::from_uuid),
        base_workflow_revision: WorkflowRevisionBoundaryValue::new(3).unwrap(),
        ordered_mutations: vec![AssistantWorkflowMutation::new(vec![1, 2, 3]).unwrap()],
        stable_aliases: AssistantWorkflowStableAliasSet::new(vec![
            AssistantWorkflowStableAliasEntry::new("hero", *Uuid::new_v4().as_bytes()).unwrap(),
        ])
        .unwrap(),
        readiness_issues: vec![],
        mutation_digest: AssistantWorkflowMutationDigest::new([7; 32]),
        resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new([8; 32]),
        lineage: AssistantWorkflowChangeLineage::UserMessage {
            invocation_id: id(AssistantModelInvocationId::from_uuid),
            intent: AssistantUserIntent::new("Create a hero image").unwrap(),
        },
        approval_scope_id: id(AssistantApprovalScopeId::from_uuid),
        expires_at: AssistantWorkflowChangeExpiry::new(20_000).unwrap(),
    })
    .unwrap()
}

fn review(
    change: &AssistantWorkflowChangeAggregate,
    verdict: AssistantReviewVerdict,
) -> AssistantReviewReceipt {
    AssistantReviewReceipt::new(
        change.id(),
        change.mutation_digest(),
        AssistantContractEpoch::new(1).unwrap(),
        AssistantModelIdentity::new("workflow_change_reviewer@1").unwrap(),
        id(AssistantModelInvocationId::from_uuid),
        AssistantToolCallId::new("call_1").unwrap(),
        verdict,
        AssistantReviewedAt::new(10_000).unwrap(),
    )
}

fn decision_scope(
    change: &AssistantWorkflowChangeAggregate,
) -> AssistantWorkflowChangeDecisionScope {
    AssistantWorkflowChangeDecisionScope {
        project_id: change.project_id(),
        session_id: change.session_id(),
        change_id: change.id(),
        approval_scope_id: change.approval_scope_id(),
        mutation_digest: change.mutation_digest(),
    }
}

#[test]
fn pass_review_then_approval_reaches_applied_only_through_applying() {
    let mut change = proposed();
    change
        .accept_review(
            review(&change, AssistantReviewVerdict::Pass),
            AssistantModelContinuationRef::new("continuation-1").unwrap(),
        )
        .unwrap();
    assert_eq!(change.state(), AssistantWorkflowChangeState::AwaitingApproval);
    change.begin_apply(decision_scope(&change), 19_999).unwrap();
    assert_eq!(change.state(), AssistantWorkflowChangeState::Applying);
    change.mark_applied().unwrap();
    assert_eq!(change.state(), AssistantWorkflowChangeState::Applied);
}

#[test]
fn reject_review_is_terminal_and_cannot_be_approved() {
    let mut change = proposed();
    change.reject_review(review(&change, AssistantReviewVerdict::Reject)).unwrap();
    assert_eq!(change.state(), AssistantWorkflowChangeState::ReviewRejected);
    assert_eq!(
        change.begin_apply(decision_scope(&change), 10_001),
        Err(AssistantWorkflowChangeError::InvalidTransition)
    );
}

#[test]
fn review_evidence_and_expiry_fail_closed() {
    let mut change = proposed();
    let wrong = AssistantReviewReceipt::new(
        change.id(),
        AssistantWorkflowMutationDigest::new([9; 32]),
        AssistantContractEpoch::new(1).unwrap(),
        AssistantModelIdentity::new("workflow_change_reviewer@1").unwrap(),
        id(AssistantModelInvocationId::from_uuid),
        AssistantToolCallId::new("call_2").unwrap(),
        AssistantReviewVerdict::Pass,
        AssistantReviewedAt::new(10_000).unwrap(),
    );
    assert_eq!(
        change.accept_review(wrong, AssistantModelContinuationRef::new("c").unwrap()),
        Err(AssistantWorkflowChangeError::ReviewEvidenceInvalid)
    );

    change
        .accept_review(
            review(&change, AssistantReviewVerdict::Pass),
            AssistantModelContinuationRef::new("c").unwrap(),
        )
        .unwrap();
    assert_eq!(
        change.begin_apply(decision_scope(&change), 20_000),
        Err(AssistantWorkflowChangeError::ApprovalExpired)
    );
    change.expire(20_000).unwrap();
    assert_eq!(change.state(), AssistantWorkflowChangeState::Expired);
}

#[test]
fn candidate_rejects_empty_mutations_duplicate_aliases_and_oversized_intent() {
    assert!(AssistantWorkflowMutation::new(Vec::new()).is_err());
    assert!(AssistantUserIntent::new("x".repeat(16 * 1024 + 1)).is_err());
    let node = *Uuid::new_v4().as_bytes();
    assert!(
        AssistantWorkflowStableAliasSet::new(vec![
            AssistantWorkflowStableAliasEntry::new("same", node).unwrap(),
            AssistantWorkflowStableAliasEntry::new("same", node).unwrap(),
        ])
        .is_err()
    );
}

#[test]
fn restore_rejects_state_without_matching_associated_evidence() {
    let change = proposed();
    let candidate = AssistantWorkflowChangeCandidate {
        id: change.id(),
        project_id: change.project_id(),
        session_id: change.session_id(),
        base_workflow_revision: change.base_workflow_revision(),
        ordered_mutations: change.ordered_mutations().to_vec(),
        stable_aliases: change.stable_aliases().clone(),
        readiness_issues: change.readiness_issues().to_vec(),
        mutation_digest: change.mutation_digest(),
        resulting_workflow_fingerprint: change.resulting_workflow_fingerprint(),
        lineage: change.lineage().clone(),
        approval_scope_id: change.approval_scope_id(),
        expires_at: change.expires_at(),
    };
    assert_eq!(
        AssistantWorkflowChangeAggregate::try_restore(
            candidate,
            None,
            None,
            AssistantWorkflowChangeState::Applying,
        ),
        Err(AssistantWorkflowChangeError::InvalidValue)
    );
}
