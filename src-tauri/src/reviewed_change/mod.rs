//! Immutable reviewed-change candidates prepared before human approval.

use engine::{
    NodeRegistry, Workflow, WorkflowPatch, WorkflowReadinessBlocker, apply_workflow_patch,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

mod approval;
mod candidate;
mod operations;
mod sqlite;

use candidate::{CandidateBase, fingerprint, new_candidate, now_seconds};
pub use operations::ReviewedChangeOperations;
pub use sqlite::ReviewedChangeSqliteRepository;

static NEXT_CANDIDATE_ID: AtomicU64 = AtomicU64::new(1);
const CANDIDATE_TTL_SECONDS: u64 = 3_600;

/// Immutable proposal built from an exact ordered patch sequence.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WorkflowCandidate {
    id: String,
    project_id: String,
    session_id: String,
    user_intent: String,
    base_revision: Option<u64>,
    patches: Vec<WorkflowPatch>,
    digest: String,
    workflow_fingerprint: String,
    workflow: Workflow,
    aliases: Vec<(String, String)>,
    readiness_blockers: Vec<WorkflowReadinessBlocker>,
    expires_at: u64,
}

impl WorkflowCandidate {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    #[must_use]
    pub fn user_intent(&self) -> &str {
        &self.user_intent
    }
    #[must_use]
    pub fn base_revision(&self) -> Option<u64> {
        self.base_revision
    }
    #[must_use]
    pub fn patches(&self) -> &[WorkflowPatch] {
        &self.patches
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
    #[must_use]
    pub fn workflow_fingerprint(&self) -> &str {
        &self.workflow_fingerprint
    }
    #[must_use]
    pub fn workflow(&self) -> &Workflow {
        &self.workflow
    }
    #[must_use]
    pub fn aliases(&self) -> &[(String, String)] {
        &self.aliases
    }
    #[must_use]
    pub fn readiness_blockers(&self) -> &[WorkflowReadinessBlocker] {
        &self.readiness_blockers
    }
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}

/// Trusted input for one bounded candidate extension.
pub struct PrepareCandidateInput {
    pub project_id: String,
    pub session_id: String,
    pub user_intent: String,
    pub expected_revision: Option<u64>,
    pub prior_candidate_id: Option<String>,
    pub patch: WorkflowPatch,
}

/// Trusted input produced by the Reviewer bridge, never by a model tool.
pub struct RecordReviewInput {
    pub project_id: String,
    pub session_id: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub reviewer_version: String,
    pub verdict: ReviewVerdict,
    pub evidence_hash: String,
    pub summary: String,
    pub findings: Vec<String>,
}

/// Persistence boundary consumed by immutable reviewed changes.
pub trait ReviewedChangeRepository: Send + Sync {
    fn insert(&self, candidate: &WorkflowCandidate) -> Result<(), ReviewedChangeError>;
    fn get(&self, candidate_id: &str) -> Result<Option<WorkflowCandidate>, ReviewedChangeError>;
    fn insert_receipt(&self, receipt: &ReviewReceipt) -> Result<(), ReviewedChangeError>;
    fn get_receipt(&self, receipt_id: &str) -> Result<Option<ReviewReceipt>, ReviewedChangeError>;
}

/// Trusted Reviewer verdict persisted outside the model tool surface.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Pass,
    Reject,
}

/// Opaque evidence that one exact candidate was reviewed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewReceipt {
    id: String,
    approval_scope_id: String,
    project_id: String,
    session_id: String,
    candidate_id: String,
    candidate_digest: String,
    reviewer_version: String,
    verdict: ReviewVerdict,
    evidence_hash: String,
    summary: String,
    findings: Vec<String>,
    expires_at: u64,
}

impl ReviewReceipt {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn approval_scope_id(&self) -> &str {
        &self.approval_scope_id
    }
    #[must_use]
    pub fn candidate_id(&self) -> &str {
        &self.candidate_id
    }
    #[must_use]
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
    #[must_use]
    pub fn verdict(&self) -> ReviewVerdict {
        self.verdict
    }
    #[must_use]
    pub fn reviewer_version(&self) -> &str {
        &self.reviewer_version
    }
    #[must_use]
    pub fn evidence_hash(&self) -> &str {
        &self.evidence_hash
    }
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
    #[must_use]
    pub fn findings(&self) -> &[String] {
        &self.findings
    }
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}

/// Read boundary for the authoritative Workflow base.
pub trait CandidateWorkflowSource: Send + Sync {
    fn load(&self, project_id: &str) -> Result<Option<(u64, Workflow)>, ReviewedChangeError>;
}

/// Prepares and retrieves immutable candidates without committing a Workflow.
pub struct ReviewedChangeService {
    registry: Arc<NodeRegistry>,
    source: Arc<dyn CandidateWorkflowSource>,
    repository: Arc<dyn ReviewedChangeRepository>,
}

impl ReviewedChangeService {
    #[must_use]
    pub fn new(
        registry: Arc<NodeRegistry>,
        source: Arc<dyn CandidateWorkflowSource>,
        repository: Arc<dyn ReviewedChangeRepository>,
    ) -> Self {
        Self { registry, source, repository }
    }

    pub fn prepare(
        &self,
        input: PrepareCandidateInput,
    ) -> Result<WorkflowCandidate, ReviewedChangeError> {
        let current = self.source.load(&input.project_id)?;
        let current_revision = current.as_ref().map(|(revision, _)| *revision);
        if input.expected_revision != current_revision {
            return Err(ReviewedChangeError::RevisionConflict {
                expected: input.expected_revision,
                actual: current_revision,
            });
        }
        let mut base = self.candidate_base(&input, current)?;
        let result = apply_workflow_patch(&self.registry, &base.workflow, &input.patch)
            .map_err(|error| ReviewedChangeError::InvalidPatch(error.to_string()))?;
        base.patches.push(input.patch.clone());
        base.aliases
            .extend(result.aliases.iter().map(|(alias, node_id)| (alias.clone(), node_id.clone())));
        let candidate = new_candidate(input, base.patches, base.aliases, result)?;
        self.repository.insert(&candidate)?;
        Ok(candidate)
    }

    pub fn get(
        &self,
        candidate_id: &str,
    ) -> Result<Option<WorkflowCandidate>, ReviewedChangeError> {
        self.repository.get(candidate_id)
    }

    pub fn record_review(
        &self,
        input: RecordReviewInput,
    ) -> Result<ReviewReceipt, ReviewedChangeError> {
        let candidate = self
            .repository
            .get(&input.candidate_id)?
            .ok_or_else(|| ReviewedChangeError::CandidateNotFound(input.candidate_id.clone()))?;
        if candidate.project_id != input.project_id
            || candidate.session_id != input.session_id
            || candidate.digest != input.candidate_digest
        {
            return Err(ReviewedChangeError::CandidateScopeMismatch);
        }
        if candidate.expires_at <= now_seconds()? {
            return Err(ReviewedChangeError::CandidateExpired);
        }
        if input.reviewer_version.trim().is_empty() || !input.evidence_hash.starts_with("sha256:") {
            return Err(ReviewedChangeError::InvalidReviewEvidence);
        }
        let receipt = new_receipt(&candidate, input)?;
        self.repository.insert_receipt(&receipt)?;
        Ok(receipt)
    }

    pub fn get_receipt(
        &self,
        receipt_id: &str,
    ) -> Result<Option<ReviewReceipt>, ReviewedChangeError> {
        self.repository.get_receipt(receipt_id)
    }

    fn candidate_base(
        &self,
        input: &PrepareCandidateInput,
        current: Option<(u64, Workflow)>,
    ) -> Result<CandidateBase, ReviewedChangeError> {
        let Some(prior_id) = input.prior_candidate_id.as_deref() else {
            let workflow = current.map(|(_, workflow)| workflow).unwrap_or_else(|| Workflow {
                version: "1.0".to_owned(),
                project_id: input.project_id.clone(),
                nodes: Vec::new(),
            });
            return Ok(CandidateBase { workflow, patches: Vec::new(), aliases: Vec::new() });
        };
        let prior = self
            .repository
            .get(prior_id)?
            .ok_or_else(|| ReviewedChangeError::CandidateNotFound(prior_id.to_owned()))?;
        if prior.project_id != input.project_id
            || prior.session_id != input.session_id
            || prior.base_revision != input.expected_revision
        {
            return Err(ReviewedChangeError::CandidateScopeMismatch);
        }
        if prior.expires_at <= now_seconds()? {
            return Err(ReviewedChangeError::CandidateExpired);
        }
        Ok(CandidateBase {
            workflow: prior.workflow,
            patches: prior.patches,
            aliases: prior.aliases,
        })
    }
}

fn new_receipt(
    candidate: &WorkflowCandidate,
    input: RecordReviewInput,
) -> Result<ReviewReceipt, ReviewedChangeError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ReviewedChangeError::Clock(error.to_string()))?
        .as_nanos();
    let sequence = NEXT_CANDIDATE_ID.fetch_add(1, Ordering::Relaxed);
    let approval_scope_id = fingerprint(&(
        candidate.session_id.as_str(),
        candidate.id.as_str(),
        candidate.digest.as_str(),
    ))?;
    Ok(ReviewReceipt {
        id: format!("review-{timestamp:032x}-{sequence:016x}"),
        approval_scope_id,
        project_id: candidate.project_id.clone(),
        session_id: candidate.session_id.clone(),
        candidate_id: candidate.id.clone(),
        candidate_digest: candidate.digest.clone(),
        reviewer_version: input.reviewer_version,
        verdict: input.verdict,
        evidence_hash: input.evidence_hash,
        summary: input.summary,
        findings: input.findings,
        expires_at: candidate.expires_at,
    })
}

#[derive(Debug, Error)]
pub enum ReviewedChangeError {
    #[error("Workflow revision conflict: expected {expected:?}, actual {actual:?}")]
    RevisionConflict { expected: Option<u64>, actual: Option<u64> },
    #[error("candidate not found: {0}")]
    CandidateNotFound(String),
    #[error("candidate scope does not match the request")]
    CandidateScopeMismatch,
    #[error("candidate has expired")]
    CandidateExpired,
    #[error("invalid Workflow patch: {0}")]
    InvalidPatch(String),
    #[error("reviewed-change storage failed: {0}")]
    Storage(String),
    #[error("system clock failed: {0}")]
    Clock(String),
    #[error("review evidence is invalid")]
    InvalidReviewEvidence,
    #[error("review receipt not found: {0}")]
    ReviewReceiptNotFound(String),
    #[error("review receipt is invalid for this approval scope")]
    ReviewReceiptInvalid,
}
