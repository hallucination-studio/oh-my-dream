use std::collections::BTreeSet;
use std::sync::Arc;

use projects::project::domain::ProjectId;

use crate::node_capability::{
    WorkflowManagedAudioRef, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeCapabilityRegistry, WorkflowRunId, WorkflowTextValue,
};
use crate::workflow_graph::{WorkflowAggregate, WorkflowId, WorkflowNodeId};

use super::use_case::check_readiness_for_nodes;
use super::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError,
    WorkflowManagedMediaPreviewSource, WorkflowMediaPreview, WorkflowMediaPreviewIssuerInterface,
    WorkflowNodeExecutionBlockReason, WorkflowNodeExecutionFailure, WorkflowNodeExecutionState,
    WorkflowReadinessIssue, WorkflowReadinessResult, WorkflowRunAggregate, WorkflowRunEvent,
    WorkflowRunEventSequence, WorkflowRunLoadKey, WorkflowRunRepositoryInterface,
};

mod stale;

use stale::is_stale;

/// One bounded ascending page of durable Run events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRunEventPage {
    /// Events strictly after the requested cursor.
    pub events: Vec<WorkflowRunEvent>,
    /// Last returned sequence only when another row exists.
    pub next_sequence: Option<WorkflowRunEventSequence>,
}

/// Latest execution facts projected for one current Workflow node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodeExecutionSummary {
    /// Producing Run.
    pub workflow_run_id: WorkflowRunId,
    /// Producing node execution.
    pub node_execution_id: crate::node_capability::WorkflowNodeExecutionId,
    /// Durable node state.
    pub state: WorkflowNodeExecutionState,
    /// Optional in-flight progress.
    pub progress_basis_points: Option<u16>,
    /// Structured failure when failed.
    pub failure: Option<WorkflowNodeExecutionFailure>,
    /// Structured block reason when blocked.
    pub block_reason: Option<WorkflowNodeExecutionBlockReason>,
    /// Frozen producing revision.
    pub producing_revision: crate::workflow_graph::WorkflowRevision,
    /// Whether the node or any ancestor differs from that revision's frozen plan.
    pub is_stale: bool,
}

/// Text presentation shell with optional latest complete value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowTextNodePresentation {
    /// Inline structured text, absent without a complete output.
    pub value: Option<WorkflowTextValue>,
}

macro_rules! media_presentation {
    ($name:ident, $value:ty, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            /// Typed managed-media value, absent without a complete output.
            pub value: Option<$value>,
            /// Short-lived preview for the producing Asset, absent without an available output.
            pub preview: Option<WorkflowMediaPreview>,
        }
    };
}

media_presentation!(
    WorkflowImageNodePresentation,
    WorkflowManagedImageRef,
    "Image presentation shell."
);
media_presentation!(
    WorkflowVideoNodePresentation,
    WorkflowManagedVideoRef,
    "Video presentation shell."
);
media_presentation!(
    WorkflowAudioNodePresentation,
    WorkflowManagedAudioRef,
    "Audio presentation shell."
);

/// Closed four-kind node presentation shell derived from the primary output contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowNodePresentationShell {
    /// Text shell.
    Text(WorkflowTextNodePresentation),
    /// Image shell.
    Image(WorkflowImageNodePresentation),
    /// Video shell.
    Video(WorkflowVideoNodePresentation),
    /// Audio shell.
    Audio(WorkflowAudioNodePresentation),
}

/// Complete current node presentation without parameters, paths, or persisted URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodePresentationView {
    /// Current node identity.
    pub node_id: WorkflowNodeId,
    /// Current Workflow revision.
    pub current_revision: crate::workflow_graph::WorkflowRevision,
    /// Exact current capability contract.
    pub capability_ref: crate::node_capability::NodeCapabilityContractRef,
    /// Current sorted readiness issues for this node.
    pub readiness_issues: Vec<WorkflowReadinessIssue>,
    /// Latest relevant execution, if any.
    pub latest_execution: Option<WorkflowNodeExecutionSummary>,
    /// Exactly one primary-output-derived shell.
    pub shell: WorkflowNodePresentationShell,
}

/// Loads one Project-scoped Workflow Run.
pub struct WorkflowGetRunUseCase<R> {
    repository: Arc<R>,
}

/// Lists one bounded page of active Project Runs.
pub struct WorkflowListActiveRunsUseCase<R> {
    repository: Arc<R>,
}

impl<R: WorkflowRunRepositoryInterface> WorkflowListActiveRunsUseCase<R> {
    /// Wires the Run repository.
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    /// Returns newest-first Queued/Running Runs with a `1..=32` bound.
    pub async fn list_active_workflow_runs(
        &self,
        project_id: ProjectId,
        limit: u8,
    ) -> Result<Vec<WorkflowRunAggregate>, WorkflowApplicationError> {
        if !(1..=32).contains(&limit) {
            return Err(WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds {
                requested_limit: u16::from(limit),
            });
        }
        self.repository.list_active_project_workflow_runs(project_id, usize::from(limit)).await
    }
}

impl<R: WorkflowRunRepositoryInterface> WorkflowGetRunUseCase<R> {
    /// Wires the Run repository.
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
    /// Loads a Run only through its owning Project.
    pub async fn get_workflow_run(
        &self,
        project_id: ProjectId,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.repository
            .load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
                project_id,
                workflow_run_id: run_id,
            })
            .await?
            .ok_or(WorkflowApplicationError::WorkflowRunNotFound)
    }
}

/// Lists one bounded durable event page.
pub struct WorkflowListRunEventsUseCase<R> {
    repository: Arc<R>,
}

impl<R: WorkflowRunRepositoryInterface> WorkflowListRunEventsUseCase<R> {
    /// Wires the Run repository.
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
    /// Returns ascending events and an exclusive continuation cursor only when more exist.
    pub async fn list_workflow_run_events(
        &self,
        project_id: ProjectId,
        run_id: WorkflowRunId,
        after_sequence: Option<WorkflowRunEventSequence>,
        limit: u16,
    ) -> Result<WorkflowRunEventPage, WorkflowApplicationError> {
        if !(1..=500).contains(&limit) {
            return Err(WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds {
                requested_limit: limit,
            });
        }
        self.repository
            .load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
                project_id,
                workflow_run_id: run_id,
            })
            .await?
            .ok_or(WorkflowApplicationError::WorkflowRunNotFound)?;
        let limit = usize::from(limit);
        let events =
            self.repository.list_workflow_run_events_after(run_id, after_sequence, limit).await?;
        let next_sequence = if events.len() == limit {
            let last = events.last().map(WorkflowRunEvent::sequence);
            match last {
                Some(sequence)
                    if !self
                        .repository
                        .list_workflow_run_events_after(run_id, Some(sequence), 1)
                        .await?
                        .is_empty() =>
                {
                    Some(sequence)
                }
                _ => None,
            }
        } else {
            None
        };
        Ok(WorkflowRunEventPage { events, next_sequence })
    }
}

/// Builds current four-kind node presentation from Workflow, Run, readiness, and preview facts.
pub struct WorkflowGetNodePresentationUseCase<A, R, P> {
    workflow_repository: Arc<A>,
    run_repository: Arc<R>,
    preview_issuer: Arc<P>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<A, R, P> WorkflowGetNodePresentationUseCase<A, R, P>
where
    A: WorkflowAggregateRepositoryInterface,
    R: WorkflowRunRepositoryInterface,
    P: WorkflowMediaPreviewIssuerInterface,
{
    /// Wires document, Run, preview, and exact capability boundaries.
    #[must_use]
    pub fn new(
        workflow_repository: Arc<A>,
        run_repository: Arc<R>,
        preview_issuer: Arc<P>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { workflow_repository, run_repository, preview_issuer, capabilities }
    }

    /// Builds one node view without exposing parameters, paths, or persisted preview values.
    pub async fn get_workflow_node_presentation(
        &self,
        project_id: ProjectId,
        workflow_id: WorkflowId,
        node_id: WorkflowNodeId,
    ) -> Result<WorkflowNodePresentationView, WorkflowApplicationError> {
        let key = super::WorkflowLoadKey::Workflow(workflow_id);
        let workflow = self
            .workflow_repository
            .load_workflow(key)
            .await?
            .filter(|workflow| workflow.project_id == project_id)
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        let node = workflow
            .nodes()
            .get(&node_id)
            .ok_or(crate::workflow_graph::WorkflowGraphError::NodeNotFound)?;
        let capability = self
            .capabilities
            .resolve_node_capability(&node.capability_contract)
            .map_err(|_| WorkflowApplicationError::WorkflowCapabilityExecutionFailure)?;
        let primary = capability
            .node_capability_contract()
            .outputs()
            .iter()
            .find(|output| output.is_primary())
            .ok_or(WorkflowApplicationError::WorkflowCapabilityExecutionFailure)?;
        let readiness = check_readiness_for_nodes(
            &workflow,
            &self.capabilities,
            Some(&BTreeSet::from([node_id])),
        )
        .await?;
        let readiness_issues = match readiness {
            WorkflowReadinessResult::Ready => Vec::new(),
            WorkflowReadinessResult::Blocked { issues } => issues,
        };
        let latest = self
            .run_repository
            .load_latest_workflow_run_for_node(project_id, workflow_id, node_id)
            .await?;
        let stale = latest
            .as_ref()
            .is_some_and(|run| is_stale(&workflow, run, node_id, &self.capabilities));
        let execution = latest.as_ref().and_then(|run| {
            run.node_executions().iter().find(|execution| execution.node_id() == node_id)
        });
        let value = execution
            .and_then(|execution| execution.outputs())
            .and_then(|outputs| outputs.get(primary.key()))
            .cloned();
        let latest_execution =
            latest.as_ref().zip(execution).map(|(run, execution)| WorkflowNodeExecutionSummary {
                workflow_run_id: run.run_id(),
                node_execution_id: execution.execution_id(),
                state: execution.state(),
                progress_basis_points: execution.progress_basis_points(),
                failure: execution.failure().cloned(),
                block_reason: execution.block_reason().cloned(),
                producing_revision: run.workflow_revision(),
                is_stale: stale,
            });
        let shell = self.shell(project_id, primary.data_type(), value).await?;
        Ok(WorkflowNodePresentationView {
            node_id,
            current_revision: workflow.revision,
            capability_ref: node.capability_contract.clone(),
            readiness_issues,
            latest_execution,
            shell,
        })
    }

    async fn shell(
        &self,
        project_id: ProjectId,
        data_type: crate::node_capability::WorkflowDataType,
        value: Option<crate::node_capability::WorkflowRuntimeValue>,
    ) -> Result<WorkflowNodePresentationShell, WorkflowApplicationError> {
        use crate::node_capability::{WorkflowDataType, WorkflowRuntimeValue};
        match (data_type, value) {
            (WorkflowDataType::Text, value) => {
                Ok(WorkflowNodePresentationShell::Text(WorkflowTextNodePresentation {
                    value: value.and_then(|value| match value {
                        WorkflowRuntimeValue::Text(value) => Some(value),
                        _ => None,
                    }),
                }))
            }
            (WorkflowDataType::Image, value) => {
                let value = value.and_then(|value| match value {
                    WorkflowRuntimeValue::Image(value) => Some(value),
                    _ => None,
                });
                let preview = self
                    .preview(project_id, value.map(WorkflowManagedMediaPreviewSource::Image))
                    .await?;
                Ok(WorkflowNodePresentationShell::Image(WorkflowImageNodePresentation {
                    value,
                    preview,
                }))
            }
            (WorkflowDataType::Video, value) => {
                let value = value.and_then(|value| match value {
                    WorkflowRuntimeValue::Video(value) => Some(value),
                    _ => None,
                });
                let preview = self
                    .preview(project_id, value.map(WorkflowManagedMediaPreviewSource::Video))
                    .await?;
                Ok(WorkflowNodePresentationShell::Video(WorkflowVideoNodePresentation {
                    value,
                    preview,
                }))
            }
            (WorkflowDataType::Audio, value) => {
                let value = value.and_then(|value| match value {
                    WorkflowRuntimeValue::Audio(value) => Some(value),
                    _ => None,
                });
                let preview = self
                    .preview(project_id, value.map(WorkflowManagedMediaPreviewSource::Audio))
                    .await?;
                Ok(WorkflowNodePresentationShell::Audio(WorkflowAudioNodePresentation {
                    value,
                    preview,
                }))
            }
        }
    }

    async fn preview(
        &self,
        project_id: ProjectId,
        source: Option<WorkflowManagedMediaPreviewSource>,
    ) -> Result<Option<WorkflowMediaPreview>, WorkflowApplicationError> {
        match source {
            Some(source) => {
                self.preview_issuer.issue_workflow_media_preview(project_id, source).await.map(Some)
            }
            None => Ok(None),
        }
    }
}
