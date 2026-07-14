use super::*;

impl ReviewedChangeService {
    pub fn replay_candidate(
        &self,
        project_id: &str,
        session_id: &str,
        receipt_id: &str,
    ) -> Result<(ReviewReceipt, WorkflowCandidate), ReviewedChangeError> {
        self.receipt_candidate(project_id, session_id, receipt_id, false)
    }

    pub fn valid_passed_receipt(
        &self,
        project_id: &str,
        session_id: &str,
        receipt_id: &str,
    ) -> Result<bool, ReviewedChangeError> {
        let Some(receipt) = self.repository.get_receipt(receipt_id)? else {
            return Ok(false);
        };
        Ok(receipt.project_id == project_id
            && receipt.session_id == session_id
            && receipt.verdict == ReviewVerdict::Pass
            && receipt.expires_at > now_seconds()?)
    }

    pub fn approved_candidate(
        &self,
        project_id: &str,
        session_id: &str,
        receipt_id: &str,
    ) -> Result<(ReviewReceipt, WorkflowCandidate), ReviewedChangeError> {
        self.receipt_candidate(project_id, session_id, receipt_id, true)
    }

    fn receipt_candidate(
        &self,
        project_id: &str,
        session_id: &str,
        receipt_id: &str,
        require_unexpired: bool,
    ) -> Result<(ReviewReceipt, WorkflowCandidate), ReviewedChangeError> {
        let receipt = self
            .repository
            .get_receipt(receipt_id)?
            .ok_or_else(|| ReviewedChangeError::ReviewReceiptNotFound(receipt_id.to_owned()))?;
        if receipt.project_id != project_id
            || receipt.session_id != session_id
            || receipt.verdict != ReviewVerdict::Pass
            || (require_unexpired && receipt.expires_at <= now_seconds()?)
        {
            return Err(ReviewedChangeError::ReviewReceiptInvalid);
        }
        let candidate = self
            .repository
            .get(&receipt.candidate_id)?
            .ok_or_else(|| ReviewedChangeError::CandidateNotFound(receipt.candidate_id.clone()))?;
        if candidate.digest != receipt.candidate_digest {
            return Err(ReviewedChangeError::ReviewReceiptInvalid);
        }
        Ok((receipt, candidate))
    }
}

impl CandidateWorkflowSource for crate::workflow_authority::WorkflowAuthority {
    fn load(&self, project_id: &str) -> Result<Option<(u64, Workflow)>, ReviewedChangeError> {
        self.load_head(project_id)
            .map(|head| head.map(|head| (head.revision, head.workflow)))
            .map_err(|error| ReviewedChangeError::Storage(error.to_string()))
    }
}
