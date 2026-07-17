//! Deterministic fakes for focused Generation Task external boundaries.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{
    GenerationTaskAssetKey, GenerationTaskAssetRecovery, GenerationTaskAvailableAsset,
    GenerationTaskBoundaryError, GenerationTaskOriginState, GenerationTaskStoreAssetCommand,
    GenerationTaskWorkflowCompletionOutcome,
};
use crate::generation_task::domain::GenerationTaskAggregate;
use crate::generation_task::interfaces::{
    GenerationTaskAssetSinkInterface, GenerationTaskOriginStateReaderInterface,
    GenerationTaskWorkflowCompletionInterface,
};

/// Fixed origin-state reader fake with observable call count.
#[derive(Clone)]
pub struct GenerationTaskOriginStateReaderFakeImpl {
    state: GenerationTaskOriginState,
    calls: Arc<Mutex<usize>>,
}

impl GenerationTaskOriginStateReaderFakeImpl {
    /// Creates a fixed origin observation.
    #[must_use]
    pub fn new(state: GenerationTaskOriginState) -> Self {
        Self { state, calls: Arc::new(Mutex::new(0)) }
    }

    /// Returns completed origin reads.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.lock().map_or(0, |calls| *calls)
    }
}

#[async_trait]
impl GenerationTaskOriginStateReaderInterface for GenerationTaskOriginStateReaderFakeImpl {
    async fn read_generation_task_origin_state(
        &self,
        _task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskOriginState, GenerationTaskBoundaryError> {
        let mut calls = self.calls.lock().map_err(|_| GenerationTaskBoundaryError::Permanent)?;
        *calls += 1;
        Ok(self.state)
    }
}

/// Fixed Asset recovery/store fake with observable call counts.
#[derive(Clone)]
pub struct GenerationTaskAssetSinkFakeImpl {
    recovery: GenerationTaskAssetRecovery,
    stored: GenerationTaskAvailableAsset,
    recovery_calls: Arc<Mutex<usize>>,
    store_calls: Arc<Mutex<usize>>,
}

impl GenerationTaskAssetSinkFakeImpl {
    /// Creates fixed recovery and storage outcomes.
    #[must_use]
    pub fn new(
        recovery: GenerationTaskAssetRecovery,
        stored: GenerationTaskAvailableAsset,
    ) -> Self {
        Self {
            recovery,
            stored,
            recovery_calls: Arc::new(Mutex::new(0)),
            store_calls: Arc::new(Mutex::new(0)),
        }
    }

    /// Returns recovery calls.
    #[must_use]
    pub fn recovery_call_count(&self) -> usize {
        self.recovery_calls.lock().map_or(0, |calls| *calls)
    }

    /// Returns storage calls.
    #[must_use]
    pub fn store_call_count(&self) -> usize {
        self.store_calls.lock().map_or(0, |calls| *calls)
    }
}

#[async_trait]
impl GenerationTaskAssetSinkInterface for GenerationTaskAssetSinkFakeImpl {
    async fn recover_generation_task_asset(
        &self,
        _key: GenerationTaskAssetKey,
    ) -> Result<GenerationTaskAssetRecovery, GenerationTaskBoundaryError> {
        let mut calls =
            self.recovery_calls.lock().map_err(|_| GenerationTaskBoundaryError::Permanent)?;
        *calls += 1;
        Ok(self.recovery.clone())
    }

    async fn store_generation_task_asset(
        &self,
        _command: GenerationTaskStoreAssetCommand,
    ) -> Result<GenerationTaskAvailableAsset, GenerationTaskBoundaryError> {
        let mut calls =
            self.store_calls.lock().map_err(|_| GenerationTaskBoundaryError::Permanent)?;
        *calls += 1;
        Ok(self.stored.clone())
    }
}

/// Fixed Workflow completion fake with observable call count.
#[derive(Clone)]
pub struct GenerationTaskWorkflowCompletionFakeImpl {
    outcome: GenerationTaskWorkflowCompletionOutcome,
    calls: Arc<Mutex<usize>>,
}

impl GenerationTaskWorkflowCompletionFakeImpl {
    /// Creates one fixed idempotent completion outcome.
    #[must_use]
    pub fn new(outcome: GenerationTaskWorkflowCompletionOutcome) -> Self {
        Self { outcome, calls: Arc::new(Mutex::new(0)) }
    }

    /// Returns completion calls.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.lock().map_or(0, |calls| *calls)
    }
}

#[async_trait]
impl GenerationTaskWorkflowCompletionInterface for GenerationTaskWorkflowCompletionFakeImpl {
    async fn complete_generation_task_workflow_origin(
        &self,
        _task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskWorkflowCompletionOutcome, GenerationTaskBoundaryError> {
        let mut calls = self.calls.lock().map_err(|_| GenerationTaskBoundaryError::Permanent)?;
        *calls += 1;
        Ok(self.outcome)
    }
}
