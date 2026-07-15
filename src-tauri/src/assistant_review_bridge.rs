//! Trusted adapter from internal Reviewer frames to reviewed-change receipts.

use crate::assistant_runtime::{
    InternalReviewHandler, InternalReviewReceipt, InternalReviewSubmission,
};
use crate::reviewed_change::{RecordReviewInput, ReviewVerdict, ReviewedChangeService};
use crate::state::AppState;
use serde::Deserialize;
use std::sync::Arc;

pub(crate) fn review_handler(state: &AppState) -> Arc<dyn InternalReviewHandler> {
    Arc::new(ReviewedChangeReviewHandler { service: Arc::clone(&state.reviewed_change) })
}

struct ReviewedChangeReviewHandler {
    service: Arc<ReviewedChangeService>,
}

impl InternalReviewHandler for ReviewedChangeReviewHandler {
    fn record(
        &self,
        project_id: &str,
        session_id: &str,
        submission: InternalReviewSubmission,
    ) -> Result<InternalReviewReceipt, String> {
        let verdict = match submission.verdict.as_str() {
            "pass" => ReviewVerdict::Pass,
            "reject" => ReviewVerdict::Reject,
            _ => return Err("review verdict is invalid".to_owned()),
        };
        let receipt = self
            .service
            .record_review(RecordReviewInput {
                project_id: project_id.to_owned(),
                session_id: session_id.to_owned(),
                candidate_id: submission.candidate_id,
                candidate_digest: submission.candidate_digest,
                reviewer_version: submission.reviewer_version,
                verdict,
                evidence_hash: submission.evidence_hash,
                summary: submission.summary,
                findings: submission.findings,
            })
            .map_err(|error| error.to_string())?;
        Ok(InternalReviewReceipt {
            candidate_id: receipt.candidate_id().to_owned(),
            review_receipt_id: receipt.id().to_owned(),
        })
    }

    fn valid_for_approval(
        &self,
        project_id: &str,
        session_id: &str,
        operation_id: &str,
        arguments_json: &str,
    ) -> Result<bool, String> {
        if operation_id != "workflow_apply_reviewed_candidate" {
            return Ok(false);
        }
        let input: ReviewedApplyInput =
            serde_json::from_str(arguments_json).map_err(|error| error.to_string())?;
        self.service
            .valid_passed_receipt(project_id, session_id, &input.review_receipt_id)
            .map_err(|error| error.to_string())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedApplyInput {
    review_receipt_id: String,
}
