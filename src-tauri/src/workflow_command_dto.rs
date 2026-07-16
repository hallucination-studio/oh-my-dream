//! Canonical Workflow command projections shared by the Desktop entry points.

use engine::{
    node_capability::NodeCapabilityParameterValue,
    workflow::{WorkflowRunAggregate, WorkflowRunEvent, WorkflowRunScope, WorkflowRunState},
    workflow_graph::{WorkflowAggregate, WorkflowInputBinding},
};
use serde::Serialize;
use serde_json::{Value, json};

/// Exact editable Workflow graph returned to React.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowDto {
    /// Current hard-cut schema version.
    pub schema_version: u16,
    /// Workflow identity.
    pub workflow_id: String,
    /// Owning Project identity.
    pub project_id: String,
    /// Non-zero revision as decimal text.
    pub revision: String,
    /// Creation UTC milliseconds as decimal text.
    pub created_at_epoch_ms: String,
    /// Latest update UTC milliseconds as decimal text.
    pub updated_at_epoch_ms: String,
    /// Nodes in stable identity order.
    pub nodes: Vec<WorkflowNodeDto>,
    /// Input bindings in stable target order.
    pub input_bindings: Vec<WorkflowInputBindingDto>,
}

/// One canonical editable node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowNodeDto {
    /// Workflow-local node identity.
    pub node_id: String,
    /// Exact capability family ID.
    pub capability_id: String,
    /// Exact `major.minor` capability version.
    pub capability_version: String,
    /// Complete D0.7 tagged parameter values.
    pub parameters: Vec<WorkflowParameterDto>,
    /// Persisted canvas position.
    pub canvas_position: WorkflowCanvasPositionDto,
}

/// One capability-owned parameter entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowParameterDto {
    /// Capability-owned key.
    pub key: String,
    /// Closed D0.7 tagged boundary value.
    pub value: Value,
}

/// Persisted finite canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct WorkflowCanvasPositionDto {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

/// One canonical target and its stable ordered input items.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowInputBindingDto {
    /// Target node identity.
    pub target_node_id: String,
    /// Target capability input key.
    pub input_key: String,
    /// `single` or `ordered_references`.
    pub kind: String,
    /// Stable input items in semantic order.
    pub items: Vec<WorkflowInputItemDto>,
}

/// One stable directed input item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowInputItemDto {
    /// Stable item identity.
    pub input_item_id: String,
    /// Source node identity.
    pub source_node_id: String,
    /// Source capability output key.
    pub source_output_key: String,
    /// Capability-owned role for ordered references, otherwise `null`.
    pub input_role_key: Option<String>,
}

/// Canonical durable Workflow Run projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowRunDto {
    /// Run identity.
    pub workflow_run_id: String,
    /// Owning Project identity.
    pub project_id: String,
    /// Frozen Workflow identity.
    pub workflow_id: String,
    /// Frozen revision as decimal text.
    pub workflow_revision: String,
    /// Closed execution scope.
    pub scope: Value,
    /// Durable state.
    pub state: String,
    /// Admission UTC milliseconds as decimal text.
    pub created_at_epoch_ms: String,
    /// Latest transition UTC milliseconds as decimal text.
    pub updated_at_epoch_ms: String,
    /// Frozen node executions in deterministic plan order.
    pub node_executions: Vec<WorkflowRunNodeExecutionDto>,
}

/// One frozen node execution identity and current durable state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowRunNodeExecutionDto {
    /// Workflow-local node identity.
    pub node_id: String,
    /// Frozen node execution identity used by durable events.
    pub node_execution_id: String,
    /// Durable execution state.
    pub state: String,
    /// Optional basis-point progress.
    pub progress_basis_points: Option<u16>,
}

/// One bounded ascending event page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowRunEventPageDto {
    /// Exact durable events.
    pub events: Vec<Value>,
    /// Exclusive continuation sequence only when more rows exist.
    pub next_sequence: Option<String>,
}

pub(crate) fn workflow_dto(workflow: &WorkflowAggregate) -> WorkflowDto {
    WorkflowDto {
        schema_version: workflow.schema_version.get(),
        workflow_id: workflow.id.as_uuid().hyphenated().to_string(),
        project_id: workflow.project_id.as_uuid().hyphenated().to_string(),
        revision: workflow.revision.get().to_string(),
        created_at_epoch_ms: workflow.created_at.as_utc_milliseconds().to_string(),
        updated_at_epoch_ms: workflow.updated_at.as_utc_milliseconds().to_string(),
        nodes: workflow.nodes().values().map(node_dto).collect(),
        input_bindings: workflow
            .input_bindings()
            .iter()
            .map(|(target, binding)| WorkflowInputBindingDto {
                target_node_id: target.node_id.as_uuid().hyphenated().to_string(),
                input_key: target.input_key.as_str().to_owned(),
                kind: match binding {
                    WorkflowInputBinding::Single { .. } => "single",
                    WorkflowInputBinding::OrderedReferences { .. } => "ordered_references",
                }
                .to_owned(),
                items: binding
                    .items()
                    .map(|item| WorkflowInputItemDto {
                        input_item_id: item.id.as_uuid().hyphenated().to_string(),
                        source_node_id: item.source_node_id.as_uuid().hyphenated().to_string(),
                        source_output_key: item.source_output_key.as_str().to_owned(),
                        input_role_key: item
                            .input_role_key
                            .as_ref()
                            .map(|key| key.as_str().to_owned()),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn node_dto(node: &engine::workflow_graph::WorkflowNodeEntity) -> WorkflowNodeDto {
    WorkflowNodeDto {
        node_id: node.id.as_uuid().hyphenated().to_string(),
        capability_id: node.capability_contract.id().as_str().to_owned(),
        capability_version: format!(
            "{}.{}",
            node.capability_contract.version().major(),
            node.capability_contract.version().minor()
        ),
        parameters: node
            .parameter_set
            .iter()
            .map(|(key, value)| WorkflowParameterDto {
                key: key.as_str().to_owned(),
                value: parameter_value(value),
            })
            .collect(),
        canvas_position: WorkflowCanvasPositionDto {
            x: node.canvas_position.x(),
            y: node.canvas_position.y(),
        },
    }
}

fn parameter_value(value: &NodeCapabilityParameterValue) -> Value {
    match value {
        NodeCapabilityParameterValue::UnsignedInteger(value) => {
            json!({"kind":"unsigned_integer","value":value.to_string()})
        }
        NodeCapabilityParameterValue::Text(value) => json!({"kind":"text","value":value}),
        NodeCapabilityParameterValue::Choice(value) => {
            json!({"kind":"choice","value":value.as_str()})
        }
        NodeCapabilityParameterValue::GenerationProfile(value) => json!({
            "kind":"generation_profile",
            "profile_id":value.profile_id(),
            "version":value.version().to_string(),
        }),
        NodeCapabilityParameterValue::ManagedAsset(value) => json!({
            "kind":"managed_asset",
            "asset_id":uuid::Uuid::from_bytes(value.asset_id().as_bytes()).hyphenated().to_string(),
        }),
    }
}

pub(crate) fn run_dto(run: &WorkflowRunAggregate) -> WorkflowRunDto {
    WorkflowRunDto {
        workflow_run_id: run.run_id().as_uuid().hyphenated().to_string(),
        project_id: run.project_id().as_uuid().hyphenated().to_string(),
        workflow_id: run.workflow_id().as_uuid().hyphenated().to_string(),
        workflow_revision: run.workflow_revision().get().to_string(),
        scope: match run.scope() {
            WorkflowRunScope::WholeWorkflow => json!({"kind":"whole_workflow"}),
            WorkflowRunScope::ThroughNode(node_id) => json!({
                "kind":"through_node","node_id":node_id.as_uuid().hyphenated().to_string()
            }),
        },
        state: run_state(run.state()).to_owned(),
        created_at_epoch_ms: run.created_at().as_utc_milliseconds().to_string(),
        updated_at_epoch_ms: run.updated_at().as_utc_milliseconds().to_string(),
        node_executions: run
            .node_executions()
            .iter()
            .map(|execution| WorkflowRunNodeExecutionDto {
                node_id: execution.node_id().as_uuid().hyphenated().to_string(),
                node_execution_id: execution.execution_id().as_uuid().hyphenated().to_string(),
                state: node_execution_state(execution.state()).to_owned(),
                progress_basis_points: execution.progress_basis_points(),
            })
            .collect(),
    }
}

pub(crate) fn event_dto(event: &WorkflowRunEvent) -> Value {
    let mut value =
        crate::assistant_workflow_bridge::run_projection::workflow_run_event_value(event);
    value["workflow_run_id"] = Value::String(event.run_id().as_uuid().hyphenated().to_string());
    value["sequence"] = Value::String(event.sequence().get().to_string());
    value["occurred_at_epoch_ms"] =
        Value::String(event.occurred_at().as_utc_milliseconds().to_string());
    value
}

const fn run_state(value: WorkflowRunState) -> &'static str {
    match value {
        WorkflowRunState::Queued => "queued",
        WorkflowRunState::Running => "running",
        WorkflowRunState::Succeeded => "succeeded",
        WorkflowRunState::Failed => "failed",
        WorkflowRunState::Cancelled => "cancelled",
    }
}

const fn node_execution_state(value: engine::workflow::WorkflowNodeExecutionState) -> &'static str {
    use engine::workflow::WorkflowNodeExecutionState;
    match value {
        WorkflowNodeExecutionState::Pending => "pending",
        WorkflowNodeExecutionState::Running => "running",
        WorkflowNodeExecutionState::Succeeded => "succeeded",
        WorkflowNodeExecutionState::Failed => "failed",
        WorkflowNodeExecutionState::Cancelled => "cancelled",
        WorkflowNodeExecutionState::Blocked => "blocked",
    }
}
