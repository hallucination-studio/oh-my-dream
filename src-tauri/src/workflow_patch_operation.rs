//! Tauri boundary for the single atomic Workflow patch operation.

use crate::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationOutputSchemaMode,
    OperationRegistration, OperationRegistrationError, RequestContext,
};
use crate::dto::WorkflowHeadDto;
use crate::state::AppState;
use crate::workflow_authority::{WorkflowAuthorityError, WorkflowCommitRequest};
use engine::{
    NodeRegistry, Workflow, WorkflowPatch, WorkflowPatchError, WorkflowPatchOperation,
    WorkflowPatchResult, WorkflowReadinessBlocker, apply_workflow_patch,
};
use nodes::SharedAssetStore;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

mod hash;
mod schema;
mod sequence;
use hash::request_hash;
#[cfg(test)]
mod tests;
/// Model-controlled input for `workflow_apply_patch`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowApplyPatchInput {
    /// Compare-and-swap revision; null creates a head only when absent.
    #[serde(default)]
    pub expected_revision: Option<u64>,
    /// Ordered closed mutation operations.
    pub operations: Vec<WorkflowPatchOperation>,
}
/// Alias resolution returned with one canonical patch acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAliasDto {
    /// Patch-local alias supplied by the caller.
    pub alias: String,
    /// Generated persisted node id.
    pub node_id: String,
}

/// Output of one atomic Workflow patch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowApplyPatchOutput {
    /// Canonical persisted head, or null when an empty patch remains absent.
    pub workflow_head: Option<WorkflowHeadDto>,
    /// Generated ids for aliases introduced by this patch.
    pub aliases: Vec<WorkflowAliasDto>,
    /// Readiness blockers retained in the acknowledged Workflow.
    pub readiness_blockers: Vec<WorkflowReadinessBlocker>,
    /// Whether the authority advanced the revision.
    pub changed: bool,
    /// Whether the authority returned an existing request receipt.
    pub deduplicated: bool,
    /// One undo unit for a changed head.
    pub undo_id: Option<String>,
}

/// Non-persisted result of evaluating one patch against the canonical head.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvaluatePatchOutput {
    /// Canonical revision used as the evaluation base.
    pub base_revision: Option<u64>,
    /// Normalized in-memory Workflow produced by the engine.
    pub workflow: Workflow,
    /// Patch-local aliases resolved during evaluation.
    pub aliases: Vec<WorkflowAliasDto>,
    /// Engine-owned execution-readiness findings.
    pub readiness_blockers: Vec<WorkflowReadinessBlocker>,
}

/// Structured failure returned by the patch boundary.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[error(
    "{code} at {pointer}: {constraint}; operation={operation_index:?}; current_revision={current_revision:?}"
)]
pub struct WorkflowApplyPatchError {
    /// Stable machine-readable code.
    pub code: String,
    /// JSON Pointer into model input or the Workflow document.
    pub pointer: String,
    /// Failing operation index, when applicable.
    pub operation_index: Option<usize>,
    /// Constraint detail.
    pub constraint: String,
    /// Revision observed before this attempt.
    pub current_revision: Option<u64>,
}

impl WorkflowApplyPatchError {
    fn new(
        code: impl Into<String>,
        pointer: impl Into<String>,
        operation_index: Option<usize>,
        constraint: impl Into<String>,
        current_revision: Option<u64>,
    ) -> Self {
        Self {
            code: code.into(),
            pointer: pointer.into(),
            operation_index,
            constraint: constraint.into(),
            current_revision,
        }
    }
}

/// Application service owning patch validation and authority composition.
pub struct WorkflowPatchService {
    registry: Arc<NodeRegistry>,
    authority: Arc<crate::workflow_authority::WorkflowAuthority>,
    store: SharedAssetStore,
}

impl WorkflowPatchService {
    /// Creates a patch service over the application composition root.
    #[must_use]
    pub fn new(
        registry: Arc<NodeRegistry>,
        authority: Arc<crate::workflow_authority::WorkflowAuthority>,
        store: SharedAssetStore,
    ) -> Self {
        Self { registry, authority, store }
    }

    /// Creates a patch service from managed app state.
    #[must_use]
    pub fn from_state(state: &AppState) -> Self {
        Self::new(
            Arc::clone(&state.registry),
            Arc::clone(&state.workflow_authority),
            Arc::clone(&state.store),
        )
    }

    /// Applies one model/UI patch using the trusted project and request scope.
    pub fn apply(
        &self,
        context: &RequestContext,
        input: WorkflowApplyPatchInput,
    ) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
        self.ensure_project(context.project_id(), None)?;
        let current = self
            .authority
            .load_head(context.project_id())
            .map_err(|error| authority_error(error, None))?;
        let current_revision = current.as_ref().map(|head| head.revision);
        let base = current
            .as_ref()
            .map(|head| head.workflow.clone())
            .unwrap_or_else(|| empty_workflow(context.project_id()));
        let patch = WorkflowPatch { operations: input.operations };
        let result = apply_workflow_patch(&self.registry, &base, &patch)
            .map_err(|error| patch_error(error, current_revision))?;
        let request_hash = request_hash(input.expected_revision, &patch).map_err(|error| {
            WorkflowApplyPatchError::new(
                "PATCH_HASH_FAILED",
                "/operations",
                None,
                error.to_string(),
                current_revision,
            )
        })?;
        let committed = self
            .authority
            .apply(WorkflowCommitRequest::new(
                context.project_id(),
                input.expected_revision,
                context.request_id(),
                request_hash,
                result.workflow.clone(),
            ))
            .map_err(|error| authority_error(error, current_revision))?;
        to_output(committed, result)
    }

    /// Evaluates a patch with engine semantics without mutating Workflow authority.
    pub fn evaluate(
        &self,
        context: &RequestContext,
        input: WorkflowApplyPatchInput,
    ) -> Result<WorkflowEvaluatePatchOutput, WorkflowApplyPatchError> {
        self.ensure_project(context.project_id(), None)?;
        let current = self
            .authority
            .load_head(context.project_id())
            .map_err(|error| authority_error(error, None))?;
        let current_revision = current.as_ref().map(|head| head.revision);
        if input.expected_revision != current_revision {
            return Err(WorkflowApplyPatchError::new(
                "WORKFLOW_REVISION_CONFLICT",
                "/expected_revision",
                None,
                format!("expected {:?}, current {current_revision:?}", input.expected_revision),
                current_revision,
            ));
        }
        let base = current
            .map(|head| head.workflow)
            .unwrap_or_else(|| empty_workflow(context.project_id()));
        let patch = WorkflowPatch { operations: input.operations };
        let result = apply_workflow_patch(&self.registry, &base, &patch)
            .map_err(|error| patch_error(error, current_revision))?;
        Ok(evaluation_output(current_revision, result))
    }

    /// Builds the sole registered model-facing Workflow mutation operation.
    pub fn operation_registration(
        self: Arc<Self>,
    ) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self);
        OperationRegistration::new_with_output_mode::<
            WorkflowApplyPatchInput,
            WorkflowApplyPatchOutput,
            _,
        >(
            "workflow_apply_patch",
            1,
            "Apply one atomic, reversible Workflow patch using exact capability contracts.",
            OperationEffect::VisibleReversibleWorkflowPatch,
            OperationInputSchemaMode::WorkflowPatchParamsOpen,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, input: WorkflowApplyPatchInput| {
                let context = context.clone();
                let service = Arc::clone(&service);
                async move { service.apply(&context, input).map_err(to_handler_error) }
            },
        )
    }

    /// Builds the model-facing non-mutating Workflow evaluation operation.
    pub fn evaluation_operation_registration(
        self: Arc<Self>,
    ) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self);
        OperationRegistration::new_with_output_mode::<
            WorkflowApplyPatchInput,
            WorkflowEvaluatePatchOutput,
            _,
        >(
            "workflow_evaluate_patch",
            1,
            "Evaluate one bounded Workflow patch without changing canonical state.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::WorkflowPatchParamsOpen,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, input: WorkflowApplyPatchInput| {
                let context = context.clone();
                let service = Arc::clone(&service);
                async move { service.evaluate(&context, input).map_err(to_handler_error) }
            },
        )
    }

    fn ensure_project(
        &self,
        project_id: &str,
        current_revision: Option<u64>,
    ) -> Result<(), WorkflowApplyPatchError> {
        let store = self.store.lock().map_err(|_| {
            WorkflowApplyPatchError::new(
                "PROJECT_STORE_UNAVAILABLE",
                "/",
                None,
                "project store lock is unavailable",
                current_revision,
            )
        })?;
        store.get_project(project_id).map(|_| ()).map_err(|error| {
            WorkflowApplyPatchError::new(
                "PROJECT_NOT_FOUND",
                "/",
                None,
                error.to_string(),
                current_revision,
            )
        })
    }
}

fn evaluation_output(
    base_revision: Option<u64>,
    result: WorkflowPatchResult,
) -> WorkflowEvaluatePatchOutput {
    WorkflowEvaluatePatchOutput {
        base_revision,
        workflow: result.workflow,
        aliases: result
            .aliases
            .into_iter()
            .map(|(alias, node_id)| WorkflowAliasDto { alias, node_id })
            .collect(),
        readiness_blockers: result.readiness_blockers,
    }
}

fn to_output(
    committed: crate::workflow_authority::WorkflowCommitResult,
    result: WorkflowPatchResult,
) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
    let aliases = result
        .aliases
        .into_iter()
        .map(|(alias, node_id)| WorkflowAliasDto { alias, node_id })
        .collect();
    to_output_parts(committed, aliases, result.readiness_blockers)
}

fn to_output_parts(
    committed: crate::workflow_authority::WorkflowCommitResult,
    aliases: Vec<WorkflowAliasDto>,
    readiness_blockers: Vec<WorkflowReadinessBlocker>,
) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
    let workflow_head =
        committed.head.map(WorkflowHeadDto::try_from).transpose().map_err(|error| {
            WorkflowApplyPatchError::new(
                "PATCH_OUTPUT_SERIALIZATION_FAILED",
                "/workflow_head",
                None,
                error.to_string(),
                None,
            )
        })?;
    Ok(WorkflowApplyPatchOutput {
        workflow_head,
        aliases,
        readiness_blockers,
        changed: committed.changed,
        deduplicated: committed.deduplicated,
        undo_id: committed.undo_id,
    })
}

fn to_handler_error(error: WorkflowApplyPatchError) -> OperationHandlerError {
    OperationHandlerError::new(error.code.clone(), error.to_string())
}

fn patch_error(
    error: WorkflowPatchError,
    current_revision: Option<u64>,
) -> WorkflowApplyPatchError {
    WorkflowApplyPatchError::new(
        error.code(),
        error.pointer(),
        error.operation_index(),
        error.constraint(),
        current_revision,
    )
}

fn authority_error(
    error: WorkflowAuthorityError,
    current_revision: Option<u64>,
) -> WorkflowApplyPatchError {
    match error {
        WorkflowAuthorityError::RevisionConflict { expected, actual } => {
            WorkflowApplyPatchError::new(
                "WORKFLOW_REVISION_CONFLICT",
                "/expected_revision",
                None,
                format!("expected {expected:?}, current {actual:?}"),
                actual,
            )
        }
        WorkflowAuthorityError::ProjectMismatch { .. } => WorkflowApplyPatchError::new(
            "WORKFLOW_PROJECT_MISMATCH",
            "/",
            None,
            error.to_string(),
            current_revision,
        ),
        WorkflowAuthorityError::RequestHashMismatch { .. } => WorkflowApplyPatchError::new(
            "WORKFLOW_REQUEST_REUSED",
            "/",
            None,
            error.to_string(),
            current_revision,
        ),
        other => WorkflowApplyPatchError::new(
            "WORKFLOW_PERSISTENCE_FAILED",
            "/",
            None,
            other.to_string(),
            current_revision,
        ),
    }
}

fn empty_workflow(project_id: &str) -> Workflow {
    Workflow { version: "1.0".to_owned(), project_id: project_id.to_owned(), nodes: Vec::new() }
}
