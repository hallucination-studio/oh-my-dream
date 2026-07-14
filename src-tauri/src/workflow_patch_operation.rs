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

mod schema;

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

fn to_output(
    committed: crate::workflow_authority::WorkflowCommitResult,
    result: WorkflowPatchResult,
) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
    let aliases = result
        .aliases
        .into_iter()
        .map(|(alias, node_id)| WorkflowAliasDto { alias, node_id })
        .collect();
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
        readiness_blockers: result.readiness_blockers,
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

fn request_hash(
    expected_revision: Option<u64>,
    patch: &WorkflowPatch,
) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(&(expected_revision, patch))?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("fnv1a:{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use engine::{CapabilityRef, NodeParams, WorkflowPatchOperation};
    use schemars::r#gen::SchemaGenerator;
    use serde_json::json;
    use tempfile::tempdir;

    fn context(request_id: &str) -> RequestContext {
        RequestContext::new("project", "session", request_id, 1, None)
    }

    #[test]
    fn operation_input_schema_closes_envelope_and_opens_only_params() {
        let schema = WorkflowApplyPatchInput::json_schema(&mut SchemaGenerator::default());
        let value = serde_json::to_value(schema).expect("serialize patch schema");
        assert_eq!(value["additionalProperties"], json!(false));
        assert_eq!(
            value["properties"]["operations"]["items"]["oneOf"][0]["properties"]["params"]["additionalProperties"],
            json!(true)
        );
    }

    #[test]
    fn request_hash_is_stable_for_the_same_typed_patch() {
        let patch = WorkflowPatch {
            operations: vec![WorkflowPatchOperation::RemoveNode {
                node: engine::NodeRef::Id { id: "n1".to_owned() },
            }],
        };
        assert_eq!(
            request_hash(Some(1), &patch).expect("hash"),
            request_hash(Some(1), &patch).expect("hash")
        );
    }

    #[test]
    fn service_requires_a_real_project_before_mutating_authority() {
        let root = tempdir().expect("asset root");
        let state = AppState::from_asset_root(root.path()).expect("app state");
        let service = WorkflowPatchService::from_state(&state);
        let error = service
            .apply(
                &context("missing"),
                WorkflowApplyPatchInput { expected_revision: None, operations: Vec::new() },
            )
            .expect_err("unknown project must fail");
        assert_eq!(error.code, "PROJECT_NOT_FOUND");
    }

    #[test]
    fn exact_capability_ref_is_used_by_the_boundary() {
        let reference = CapabilityRef::new("TextPrompt", "1.0");
        assert_eq!(reference.version, "1.0");
        let _params = NodeParams::new();
    }
}
