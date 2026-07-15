//! Pure, ordered Workflow patch application and validation.

use crate::capability::CapabilityRef;
use crate::error::EngineError;
use crate::graph::{InputBinding, Workflow, WorkflowNode};
use crate::registry::{NodeParams, NodeRegistry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod operations;
mod validation;

use operations::{apply_operation, normalize_workflow};
pub use validation::{
    WorkflowDiagnostic, WorkflowReadinessBlocker, WorkflowValidationReport, validate_workflow,
};

/// Maximum number of operations accepted in one patch.
pub const MAX_WORKFLOW_PATCH_OPERATIONS: usize = 128;
/// Maximum serialized patch size accepted at the engine boundary.
pub const MAX_WORKFLOW_PATCH_BYTES: usize = 512 * 1024;

/// A model or UI reference to an existing or patch-local node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeRef {
    /// Stable persisted node id.
    Id { id: String },
    /// Alias introduced by an earlier add operation in this patch.
    Alias { alias: String },
}

/// A patch source that names both its node and exact declared output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchOutputRef {
    /// Existing or patch-local source node.
    pub node: NodeRef,
    /// Exact output port declared by the source capability.
    pub output: String,
}

/// One closed, ordered Workflow mutation operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowPatchOperation {
    /// Adds one exact capability node and exposes it under a patch-local alias.
    AddNode {
        /// Alias used by later operations in this patch.
        alias: String,
        /// Exact capability identity; no current-version fallback is allowed.
        capability: CapabilityRef,
        /// Complete capability params object.
        params: NodeParams,
        /// Optional canvas position.
        position: Option<[f64; 2]>,
    },
    /// Replaces the complete normalized params object of a node.
    ReplaceParams { node: NodeRef, params: NodeParams },
    /// Sets one explicit single or ordered-many input binding.
    SetInput { node: NodeRef, input: String, binding: InputBinding<PatchOutputRef> },
    /// Removes an input binding while retaining the node.
    ClearInput { node: NodeRef, input: String },
    /// Removes a node and every incident input binding.
    RemoveNode { node: NodeRef },
    /// Changes only the UI position of a node.
    SetPosition { node: NodeRef, position: [f64; 2] },
}

/// An ordered patch independent of project scope and request identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPatch {
    /// Operations applied from first to last.
    pub operations: Vec<WorkflowPatchOperation>,
}

/// Canonical result of applying a patch to a Workflow copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPatchResult {
    /// Normalized candidate Workflow.
    pub workflow: Workflow,
    /// Resolved patch-local aliases and their generated persisted ids.
    pub aliases: BTreeMap<String, String>,
    /// Missing inputs and cardinality blockers retained for the UI.
    pub readiness_blockers: Vec<WorkflowReadinessBlocker>,
}

/// Patch application failure before the Workflow authority is called.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{diagnostic_code} at {pointer}: {constraint}")]
pub struct WorkflowPatchError {
    diagnostic_code: String,
    pointer: String,
    constraint: String,
    operation_index: Option<usize>,
}

impl WorkflowPatchError {
    /// Creates a patch error with an optional operation index.
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        pointer: impl Into<String>,
        constraint: impl Into<String>,
        operation_index: Option<usize>,
    ) -> Self {
        Self {
            diagnostic_code: code.into(),
            pointer: pointer.into(),
            constraint: constraint.into(),
            operation_index,
        }
    }

    /// Returns the stable error code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.diagnostic_code
    }

    /// Returns the JSON Pointer.
    #[must_use]
    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    /// Returns the constraint detail.
    #[must_use]
    pub fn constraint(&self) -> &str {
        &self.constraint
    }

    /// Returns the failing operation index, when the failure came from an op.
    #[must_use]
    pub fn operation_index(&self) -> Option<usize> {
        self.operation_index
    }
}

/// Applies an ordered patch to a copy and validates its persistable result.
pub fn apply_workflow_patch(
    registry: &NodeRegistry,
    workflow: &Workflow,
    patch: &WorkflowPatch,
) -> Result<WorkflowPatchResult, WorkflowPatchError> {
    check_patch_size(patch)?;
    let mut candidate = normalize_workflow(registry, workflow.clone())?;
    let mut aliases = BTreeMap::new();
    for (index, operation) in patch.operations.iter().enumerate() {
        apply_operation(registry, &mut candidate, &mut aliases, operation, index)?;
    }

    let normalized = normalize_workflow(registry, candidate)?;
    let report = validate_workflow(registry, &normalized);
    if let Some(error) = report.persistence_errors.first() {
        return Err(WorkflowPatchError::new(&error.code, &error.pointer, &error.constraint, None));
    }
    Ok(WorkflowPatchResult {
        workflow: normalized,
        aliases,
        readiness_blockers: report.readiness_blockers,
    })
}

fn check_patch_size(patch: &WorkflowPatch) -> Result<(), WorkflowPatchError> {
    if patch.operations.len() > MAX_WORKFLOW_PATCH_OPERATIONS {
        return Err(WorkflowPatchError::new(
            "PATCH_OPERATION_LIMIT",
            "/operations",
            format!("at most {MAX_WORKFLOW_PATCH_OPERATIONS} operations are allowed"),
            None,
        ));
    }
    let bytes = serde_json::to_vec(patch).map_err(|error| {
        WorkflowPatchError::new("PATCH_ENCODING_FAILED", "/", error.to_string(), None)
    })?;
    if bytes.len() > MAX_WORKFLOW_PATCH_BYTES {
        return Err(WorkflowPatchError::new(
            "PATCH_SIZE_LIMIT",
            "/operations",
            format!("serialized patch must be at most {MAX_WORKFLOW_PATCH_BYTES} bytes"),
            None,
        ));
    }
    Ok(())
}

fn instantiate(
    registry: &NodeRegistry,
    node: &WorkflowNode,
    index: usize,
    pointer: &str,
) -> Result<Box<dyn crate::Node>, WorkflowPatchError> {
    registry
        .instantiate_workflow_node(&node.id, &node.type_id, &node.contract_version, &node.params)
        .map_err(|error| {
            let diagnostic = engine_diagnostic(error, pointer);
            WorkflowPatchError::new(
                diagnostic.code,
                diagnostic.pointer,
                diagnostic.constraint,
                Some(index),
            )
        })
}

fn diagnostic(
    code: impl Into<String>,
    pointer: impl Into<String>,
    constraint: impl Into<String>,
) -> WorkflowDiagnostic {
    WorkflowDiagnostic { code: code.into(), pointer: pointer.into(), constraint: constraint.into() }
}

fn indexed_error(
    code: impl Into<String>,
    pointer: impl Into<String>,
    constraint: impl Into<String>,
    index: usize,
) -> WorkflowPatchError {
    let pointer = pointer.into();
    let pointer = if pointer == "/" {
        format!("/operations/{index}")
    } else {
        format!("/operations/{index}{pointer}")
    };
    WorkflowPatchError::new(code, pointer, constraint, Some(index))
}

fn engine_diagnostic(error: EngineError, pointer: &str) -> WorkflowDiagnostic {
    let (code, constraint) = match error {
        EngineError::UnknownCapabilityVersion { .. } => (
            "CAPABILITY_VERSION_UNAVAILABLE",
            "exact capability version is not registered".to_owned(),
        ),
        EngineError::InvalidCapabilityParams { source, .. } => {
            ("CAPABILITY_PARAMS_INVALID", source.to_string())
        }
        EngineError::InvalidCapabilitySelector { reason, .. } => {
            ("CAPABILITY_PARAMS_INVALID", reason)
        }
        EngineError::CapabilityContractMismatch { message, .. } => {
            ("CAPABILITY_CONTRACT_INVALID", message)
        }
        other => ("WORKFLOW_INVALID", other.to_string()),
    };
    diagnostic(code, pointer, constraint)
}

impl WorkflowPatchError {
    fn diagnostic(&self) -> WorkflowDiagnostic {
        diagnostic(self.code(), self.pointer(), self.constraint())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeRegistry;

    fn empty_workflow() -> Workflow {
        Workflow { version: "1.0".to_owned(), project_id: "project".to_owned(), nodes: Vec::new() }
    }

    #[test]
    fn patch_adds_and_replaces_using_later_aliases_only() {
        let registry = NodeRegistry::new();
        let patch = WorkflowPatch {
            operations: vec![WorkflowPatchOperation::AddNode {
                alias: "prompt".to_owned(),
                capability: CapabilityRef::new("missing", "1.0"),
                params: NodeParams::new(),
                position: None,
            }],
        };
        let error = apply_workflow_patch(&registry, &empty_workflow(), &patch)
            .expect_err("unknown exact capability must fail");
        assert_eq!(error.code(), "CAPABILITY_VERSION_UNAVAILABLE");
        assert_eq!(error.operation_index(), Some(0));
    }

    #[test]
    fn patch_limits_operations_before_mutating_the_copy() {
        let patch = WorkflowPatch {
            operations: (0..=MAX_WORKFLOW_PATCH_OPERATIONS)
                .map(|_| WorkflowPatchOperation::ClearInput {
                    node: NodeRef::Id { id: "n".to_owned() },
                    input: "x".to_owned(),
                })
                .collect(),
        };
        let error = check_patch_size(&patch).expect_err("operation limit must fail");
        assert_eq!(error.code(), "PATCH_OPERATION_LIMIT");
    }
}
