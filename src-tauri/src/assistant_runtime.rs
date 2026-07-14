mod command;
mod dispatch;
mod error;
mod frames;
mod payload;
mod process;
mod runner;
mod types;

use std::collections::HashMap;
use std::sync::Arc;

use crate::assistant_operations::OperationRegistration;
use serde::Deserialize;

pub use command::AssistantSidecarCommand;
pub use error::AssistantRuntimeError;
pub use process::{AssistantProcess, AssistantProcessLauncher};
pub use types::{
    AssistantCompleted, AssistantEventSink, AssistantInvocation, AssistantPendingApproval,
    AssistantRuntimeLimits, AssistantRuntimeOutcome, AssistantSessionSnapshot,
    AssistantWaitingApproval, OperationCallEvidence, TrustedInvocationContext,
};

/// Application-owned runtime that dispatches sidecar tool requests through Rust registrations.
pub struct AssistantRuntime {
    pub(super) launcher: Arc<dyn AssistantProcessLauncher>,
    pub(super) limits: AssistantRuntimeLimits,
    pub(super) registrations: Vec<OperationRegistration>,
    pub(super) registrations_by_id: HashMap<String, usize>,
    pub(super) review_handler: Option<Arc<dyn InternalReviewHandler>>,
}

/// Attested nested Reviewer result accepted only through the internal protocol.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InternalReviewSubmission {
    pub invocation_id: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub reviewer_version: String,
    pub verdict: String,
    pub summary: String,
    pub findings: Vec<String>,
    pub evidence_hash: String,
}

/// Result returned to the Reviewer wrapper after trusted persistence.
pub struct InternalReviewReceipt {
    pub candidate_id: String,
    pub review_receipt_id: String,
}

/// Consumer-owned boundary for non-model review receipt persistence.
pub trait InternalReviewHandler: Send + Sync {
    fn record(
        &self,
        project_id: &str,
        session_id: &str,
        submission: InternalReviewSubmission,
    ) -> Result<InternalReviewReceipt, String>;
}

impl AssistantRuntime {
    /// Creates a runtime and rejects ambiguous operation IDs.
    pub fn new<L>(
        launcher: L,
        registrations: Vec<OperationRegistration>,
    ) -> Result<Self, AssistantRuntimeError>
    where
        L: AssistantProcessLauncher + 'static,
    {
        Self::with_limits(launcher, registrations, AssistantRuntimeLimits::default())
    }

    /// Creates a runtime with explicit resource limits.
    pub fn with_limits<L>(
        launcher: L,
        registrations: Vec<OperationRegistration>,
        limits: AssistantRuntimeLimits,
    ) -> Result<Self, AssistantRuntimeError>
    where
        L: AssistantProcessLauncher + 'static,
    {
        let mut registrations_by_id = HashMap::new();
        for (index, registration) in registrations.iter().enumerate() {
            if registrations_by_id.insert(registration.id().to_owned(), index).is_some() {
                return Err(AssistantRuntimeError::DuplicateOperation {
                    operation_id: registration.id().to_owned(),
                });
            }
        }
        Ok(Self {
            launcher: Arc::new(launcher),
            limits,
            registrations,
            registrations_by_id,
            review_handler: None,
        })
    }

    /// Installs the trusted internal Reviewer receipt boundary.
    #[must_use]
    pub fn with_review_handler(mut self, handler: Arc<dyn InternalReviewHandler>) -> Self {
        self.review_handler = Some(handler);
        self
    }

    /// Launches a fresh sidecar and runs one new invocation.
    pub async fn invoke(
        &self,
        invocation: AssistantInvocation,
        trusted: TrustedInvocationContext,
    ) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
        let mut sink = types::NoopEventSink;
        runner::invoke(self, invocation, trusted, &mut sink).await
    }

    /// Launches an invocation and emits safe lifecycle events to the caller.
    pub async fn invoke_streamed(
        &self,
        invocation: AssistantInvocation,
        trusted: TrustedInvocationContext,
        sink: &mut dyn AssistantEventSink,
    ) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
        runner::invoke(self, invocation, trusted, sink).await
    }

    /// Launches a fresh sidecar and resumes one opaque pending approval state.
    pub async fn resume(
        &self,
        invocation: AssistantInvocation,
        trusted: TrustedInvocationContext,
        waiting: AssistantWaitingApproval,
        approved: bool,
    ) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
        let mut sink = types::NoopEventSink;
        runner::resume(self, invocation, trusted, waiting, approved, &mut sink).await
    }

    /// Resumes an invocation and emits safe lifecycle events to the caller.
    pub async fn resume_streamed(
        &self,
        invocation: AssistantInvocation,
        trusted: TrustedInvocationContext,
        waiting: AssistantWaitingApproval,
        approved: bool,
        sink: &mut dyn AssistantEventSink,
    ) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
        runner::resume(self, invocation, trusted, waiting, approved, sink).await
    }

    pub(super) fn registration(
        &self,
        operation_id: &str,
    ) -> Result<&OperationRegistration, AssistantRuntimeError> {
        self.registrations_by_id
            .get(operation_id)
            .map(|index| &self.registrations[*index])
            .ok_or_else(|| AssistantRuntimeError::UnknownOperation {
                operation_id: operation_id.to_owned(),
            })
    }
}
