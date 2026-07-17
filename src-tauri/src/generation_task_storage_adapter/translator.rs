use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    WorkflowNodeExecutionId, WorkflowRunId,
};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use projects::project::domain::ProjectId;
use rusqlite::Row;
use tasks::generation_task::{application::GenerationTaskStatus, domain::*};
use uuid::Uuid;

#[path = "translator/request.rs"]
mod request;
#[path = "translator/state.rs"]
mod state;

#[derive(Debug)]
pub(super) struct TaskRow {
    pub id: Vec<u8>,
    pub project_id: Vec<u8>,
    pub workflow_id: Vec<u8>,
    pub workflow_revision: i64,
    pub workflow_run_id: Vec<u8>,
    pub workflow_node_id: Vec<u8>,
    pub workflow_node_execution_id: Vec<u8>,
    pub capability_contract_id: String,
    pub capability_contract_major: i64,
    pub capability_contract_minor: i64,
    pub idempotency_key: Vec<u8>,
    pub request_hash: Vec<u8>,
    pub request_schema_version: i64,
    pub request_kind: String,
    pub request_json: String,
    pub generation_profile_ref: String,
    pub provider_id: String,
    pub route_id: String,
    pub status: String,
    pub progress_percent: Option<i64>,
    pub remote_task_id: Option<String>,
    pub result_kind: Option<String>,
    pub result_text: Option<String>,
    pub result_asset_id: Option<Vec<u8>>,
    pub result_media_kind: Option<String>,
    pub failure_kind: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub provider_deadline_at: i64,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

impl TaskRow {
    pub(super) fn from_sql(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            workflow_id: row.get("workflow_id")?,
            workflow_revision: row.get("workflow_revision")?,
            workflow_run_id: row.get("workflow_run_id")?,
            workflow_node_id: row.get("workflow_node_id")?,
            workflow_node_execution_id: row.get("workflow_node_execution_id")?,
            capability_contract_id: row.get("capability_contract_id")?,
            capability_contract_major: row.get("capability_contract_major")?,
            capability_contract_minor: row.get("capability_contract_minor")?,
            idempotency_key: row.get("idempotency_key")?,
            request_hash: row.get("request_hash")?,
            request_schema_version: row.get("request_schema_version")?,
            request_kind: row.get("request_kind")?,
            request_json: row.get("request_json")?,
            generation_profile_ref: row.get("generation_profile_ref")?,
            provider_id: row.get("provider_id")?,
            route_id: row.get("route_id")?,
            status: row.get("status")?,
            progress_percent: row.get("progress_percent")?,
            remote_task_id: row.get("remote_task_id")?,
            result_kind: row.get("result_kind")?,
            result_text: row.get("result_text")?,
            result_asset_id: row.get("result_asset_id")?,
            result_media_kind: row.get("result_media_kind")?,
            failure_kind: row.get("failure_kind")?,
            failure_code: row.get("failure_code")?,
            failure_message: row.get("failure_message")?,
            provider_deadline_at: row.get("provider_deadline_at")?,
            completed_at: row.get("completed_at")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            revision: row.get("revision")?,
        })
    }

    pub(super) fn restore(self) -> Result<GenerationTaskAggregate, ()> {
        if self.request_schema_version != 1 {
            return Err(());
        }
        let id = task_id(&self.id)?;
        let origin = GenerationTaskOrigin::new(
            project_id(&self.project_id)?,
            workflow_id(&self.workflow_id)?,
            WorkflowRevision::new(u64::try_from(self.workflow_revision).map_err(|_| ())?)
                .map_err(|_| ())?,
            workflow_run_id(&self.workflow_run_id)?,
            workflow_node_id(&self.workflow_node_id)?,
            workflow_node_execution_id(&self.workflow_node_execution_id)?,
            NodeCapabilityContractRef::new(
                NodeCapabilityContractId::new(self.capability_contract_id.clone())
                    .map_err(|_| ())?,
                NodeCapabilityContractVersion::new(
                    u16::try_from(self.capability_contract_major).map_err(|_| ())?,
                    u16::try_from(self.capability_contract_minor).map_err(|_| ())?,
                )
                .map_err(|_| ())?,
            ),
        );
        let request = request::decode(&self.request_json)?;
        if kind(request.kind()) != self.request_kind {
            return Err(());
        }
        let task_state = state::decode_state(&self, self.result_kind.is_some())?;
        let target = GenerationTaskTarget::new(
            profile_ref(&self.generation_profile_ref)?,
            GenerationProviderId::try_new(self.provider_id).map_err(|_| ())?,
            GenerationProviderRouteId::try_new(self.route_id).map_err(|_| ())?,
        );
        let result = state::decode_result(
            self.result_kind.as_deref(),
            self.result_text,
            self.result_asset_id,
            self.result_media_kind.as_deref(),
        )?;
        GenerationTaskAggregate::restore(
            id,
            origin,
            GenerationTaskIdempotencyKey::try_new(
                String::from_utf8(self.idempotency_key).map_err(|_| ())?,
            )
            .map_err(|_| ())?,
            GenerationTaskRequestHash::from_bytes(array::<32>(&self.request_hash)?),
            target,
            request,
            timestamp(self.provider_deadline_at)?,
            task_state,
            result,
            timestamp(self.created_at)?,
            timestamp(self.updated_at)?,
            GenerationTaskRevision::try_new(u64::try_from(self.revision).map_err(|_| ())?)
                .map_err(|_| ())?,
        )
        .map_err(|_| ())
    }
}

pub(super) struct EncodedTask {
    pub request_kind: &'static str,
    pub request_json: String,
    pub status: &'static str,
    pub progress_percent: Option<i64>,
    pub remote_task_id: Option<String>,
    pub result_kind: Option<&'static str>,
    pub result_text: Option<String>,
    pub result_asset_id: Option<Vec<u8>>,
    pub result_media_kind: Option<&'static str>,
    pub failure_kind: Option<&'static str>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub completed_at: Option<i64>,
}

pub(super) fn encode_task(task: &GenerationTaskAggregate) -> Result<EncodedTask, ()> {
    let state = state::encode(task.state(), task.result());
    Ok(EncodedTask {
        request_kind: kind(task.request().kind()),
        request_json: request::encode(task.request())?,
        status: state.status,
        progress_percent: state.progress_percent,
        remote_task_id: state.remote_task_id,
        result_kind: state.result_kind,
        result_text: state.result_text,
        result_asset_id: state.result_asset_id,
        result_media_kind: state.result_media_kind,
        failure_kind: state.failure_kind,
        failure_code: state.failure_code,
        failure_message: state.failure_message,
        completed_at: state.completed_at,
    })
}

pub(super) const fn status(status: GenerationTaskStatus) -> &'static str {
    match status {
        GenerationTaskStatus::Queued => "Queued",
        GenerationTaskStatus::Running => "Running",
        GenerationTaskStatus::CancelRequested => "CancelRequested",
        GenerationTaskStatus::Succeeded => "Succeeded",
        GenerationTaskStatus::Failed => "Failed",
        GenerationTaskStatus::Cancelled => "Cancelled",
    }
}

pub(super) const fn kind(kind: GenerationTaskRequestKind) -> &'static str {
    match kind {
        GenerationTaskRequestKind::Text => "Text",
        GenerationTaskRequestKind::Image => "Image",
        GenerationTaskRequestKind::Video => "Video",
        GenerationTaskRequestKind::Voice => "Voice",
    }
}

fn profile_ref(value: &str) -> Result<GenerationProfileRef, ()> {
    let (id, version) = value.rsplit_once('@').ok_or(())?;
    Ok(GenerationProfileRef::new(
        GenerationProfileId::try_new(id).map_err(|_| ())?,
        GenerationProfileVersion::try_new(version.parse().map_err(|_| ())?).map_err(|_| ())?,
    ))
}

pub(super) fn timestamp(value: i64) -> Result<GenerationTaskTimestamp, ()> {
    GenerationTaskTimestamp::from_utc_milliseconds(value).map_err(|_| ())
}

pub(super) fn uuid(value: &[u8]) -> Result<Uuid, ()> {
    Ok(Uuid::from_bytes(array::<16>(value)?))
}

fn task_id(value: &[u8]) -> Result<GenerationTaskId, ()> {
    GenerationTaskId::from_uuid(uuid(value)?).map_err(|_| ())
}
fn project_id(value: &[u8]) -> Result<ProjectId, ()> {
    ProjectId::from_uuid(uuid(value)?).ok_or(())
}
fn workflow_id(value: &[u8]) -> Result<WorkflowId, ()> {
    WorkflowId::from_uuid(uuid(value)?).map_err(|_| ())
}
fn workflow_run_id(value: &[u8]) -> Result<WorkflowRunId, ()> {
    WorkflowRunId::from_uuid(uuid(value)?).ok_or(())
}
fn workflow_node_id(value: &[u8]) -> Result<WorkflowNodeId, ()> {
    WorkflowNodeId::from_uuid(uuid(value)?).map_err(|_| ())
}
fn workflow_node_execution_id(value: &[u8]) -> Result<WorkflowNodeExecutionId, ()> {
    WorkflowNodeExecutionId::from_uuid(uuid(value)?).ok_or(())
}

pub(super) fn array<const N: usize>(value: &[u8]) -> Result<[u8; N], ()> {
    value.try_into().map_err(|_| ())
}
