use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assistant_operations::OperationContract;

#[derive(Serialize)]
pub(super) struct InvokePayload<'a> {
    pub(super) invocation_id: &'a str,
    pub(super) session_id: &'a str,
    pub(super) session_path: &'a str,
    pub(super) input: Option<&'a str>,
    pub(super) operations: Vec<OperationContract>,
    pub(super) state: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResponsesEventPayload {
    pub(super) invocation_id: String,
    pub(super) event: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ToolRequestPayload {
    pub(super) invocation_id: String,
    pub(super) operation_id: String,
    pub(super) call_id: String,
    pub(super) arguments_json: String,
}

#[derive(Serialize)]
pub(super) struct ToolResponsePayload<'a> {
    pub(super) invocation_id: &'a str,
    pub(super) call_id: &'a str,
    pub(super) output_json: &'a str,
}

#[derive(Serialize)]
pub(super) struct ReviewResponsePayload<'a> {
    pub(super) invocation_id: &'a str,
    pub(super) candidate_id: &'a str,
    pub(super) review_receipt_id: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ApprovalRequestPayload {
    pub(super) invocation_id: String,
    pub(super) operation_id: String,
    pub(super) call_id: String,
    pub(super) arguments_json: String,
    pub(super) state: Value,
}

#[derive(Serialize)]
pub(super) struct ApprovalResponsePayload<'a> {
    pub(super) invocation_id: &'a str,
    pub(super) call_id: &'a str,
    pub(super) approved: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SnapshotPayload {
    pub(super) invocation_id: String,
    pub(super) session_id: String,
    pub(super) status: String,
    pub(super) state: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletedPayload {
    pub(super) invocation_id: String,
    pub(super) final_output: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ErrorPayload {
    pub(super) invocation_id: String,
    pub(super) code: String,
    pub(super) message: String,
}
