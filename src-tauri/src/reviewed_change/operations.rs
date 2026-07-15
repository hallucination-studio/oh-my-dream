use super::{PrepareCandidateInput, ReviewedChangeError, ReviewedChangeService, WorkflowCandidate};
use crate::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationOutputSchemaMode,
    OperationRegistration, OperationRegistrationError, RequestContext,
};
use crate::workflow_patch_operation::{
    WorkflowAliasDto, WorkflowApplyPatchInput, WorkflowApplyPatchOutput, WorkflowPatchService,
};
use engine::{WorkflowPatch, WorkflowPatchOperation};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparePatchInput {
    #[serde(default)]
    expected_revision: Option<u64>,
    #[serde(default)]
    prior_candidate_id: Option<String>,
    operations: Vec<WorkflowPatchOperation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetCandidateInput {
    candidate_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApplyReviewedCandidateInput {
    review_receipt_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CandidateOutput {
    candidate_id: String,
    project_id: String,
    user_intent: String,
    #[schemars(required)]
    base_revision: Option<u64>,
    patch_count: usize,
    patches: Value,
    digest: String,
    workflow_fingerprint: String,
    workflow: Value,
    readiness_blockers: Vec<ReadinessBlockerDto>,
    expires_at: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadinessBlockerDto {
    code: String,
    pointer: String,
    constraint: String,
}

pub struct ReviewedChangeOperations {
    service: Arc<ReviewedChangeService>,
    patch_service: Arc<WorkflowPatchService>,
}

impl ReviewedChangeOperations {
    #[must_use]
    pub fn new(
        service: Arc<ReviewedChangeService>,
        patch_service: Arc<WorkflowPatchService>,
    ) -> Self {
        Self { service, patch_service }
    }

    pub fn registrations(self) -> Result<Vec<OperationRegistration>, OperationRegistrationError> {
        Ok(vec![self.prepare_registration()?, self.get_registration()?, self.apply_registration()?])
    }

    fn prepare_registration(&self) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self.service);
        OperationRegistration::new_with_output_mode::<PreparePatchInput, CandidateOutput, _>(
            "workflow_prepare_patch",
            2,
            "Prepare an immutable Workflow candidate without changing canonical state.",
            OperationEffect::AssistantStateMutation,
            OperationInputSchemaMode::WorkflowPatchParamsOpen,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, input: PreparePatchInput| {
                let service = Arc::clone(&service);
                let context = context.clone();
                async move {
                    let candidate = service
                        .prepare(PrepareCandidateInput {
                            project_id: context.project_id().to_owned(),
                            session_id: context.session_id().to_owned(),
                            user_intent: context.user_request().unwrap_or_default().to_owned(),
                            expected_revision: input.expected_revision,
                            prior_candidate_id: input.prior_candidate_id,
                            patch: WorkflowPatch { operations: input.operations },
                        })
                        .map_err(handler_error)?;
                    CandidateOutput::from_candidate(&candidate).map_err(handler_error)
                }
            },
        )
    }

    fn get_registration(&self) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self.service);
        OperationRegistration::new_with_output_mode::<GetCandidateInput, CandidateOutput, _>(
            "workflow_candidate_get",
            2,
            "Read one exact immutable Workflow candidate by opaque ID.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, input: GetCandidateInput| {
                let service = Arc::clone(&service);
                let project_id = context.project_id().to_owned();
                let session_id = context.session_id().to_owned();
                async move {
                    let candidate = service
                        .get(&input.candidate_id)
                        .map_err(handler_error)?
                        .ok_or_else(|| {
                            handler_error(ReviewedChangeError::CandidateNotFound(
                                input.candidate_id,
                            ))
                        })?;
                    if candidate.project_id() != project_id || candidate.session_id() != session_id
                    {
                        return Err(handler_error(ReviewedChangeError::CandidateScopeMismatch));
                    }
                    CandidateOutput::from_candidate(&candidate).map_err(handler_error)
                }
            },
        )
    }

    fn apply_registration(&self) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self.service);
        let patch_service = Arc::clone(&self.patch_service);
        OperationRegistration::new_with_output_mode::<
            ApplyReviewedCandidateInput,
            WorkflowApplyPatchOutput,
            _,
        >(
            "workflow_apply_reviewed_candidate",
            2,
            "Apply one passed and human-approved immutable Workflow candidate.",
            OperationEffect::PreparedApprovalExecution,
            OperationInputSchemaMode::Strict,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, input: ApplyReviewedCandidateInput| {
                let service = Arc::clone(&service);
                let patch_service = Arc::clone(&patch_service);
                let context = context.clone();
                async move {
                    if context.approved_effect().is_none() {
                        return Err(OperationHandlerError::new(
                            "REVIEW_APPROVAL_REQUIRED",
                            "reviewed candidate requires trusted human approval",
                        ));
                    }
                    let (replay_receipt, replay_candidate) = service
                        .replay_candidate(
                            context.project_id(),
                            context.session_id(),
                            &input.review_receipt_id,
                        )
                        .map_err(handler_error)?;
                    let replay_context = RequestContext::new(
                        context.project_id(),
                        context.session_id(),
                        replay_receipt.approval_scope_id(),
                        1,
                        None,
                    );
                    if let Some(output) = patch_service
                        .replay_sequence(
                            &replay_context,
                            replay_candidate.base_revision(),
                            replay_candidate.patches(),
                            replay_candidate
                                .aliases()
                                .iter()
                                .map(|(alias, node_id)| WorkflowAliasDto {
                                    alias: alias.clone(),
                                    node_id: node_id.clone(),
                                })
                                .collect(),
                            replay_candidate.readiness_blockers().to_vec(),
                        )
                        .map_err(|error| {
                            OperationHandlerError::new(error.code.clone(), error.to_string())
                        })?
                    {
                        return Ok(output);
                    }
                    let (receipt, candidate) = service
                        .approved_candidate(
                            context.project_id(),
                            context.session_id(),
                            &input.review_receipt_id,
                        )
                        .map_err(handler_error)?;
                    let apply_context = RequestContext::new(
                        context.project_id(),
                        context.session_id(),
                        receipt.approval_scope_id(),
                        1,
                        None,
                    );
                    patch_service
                        .apply_sequence(
                            &apply_context,
                            candidate.base_revision(),
                            candidate.patches(),
                            candidate.workflow_fingerprint(),
                        )
                        .map_err(|error| {
                            OperationHandlerError::new(error.code.clone(), error.to_string())
                        })
                }
            },
        )
    }
}

impl CandidateOutput {
    fn from_candidate(candidate: &WorkflowCandidate) -> Result<Self, ReviewedChangeError> {
        Ok(Self {
            candidate_id: candidate.id().to_owned(),
            project_id: candidate.project_id().to_owned(),
            user_intent: candidate.user_intent().to_owned(),
            base_revision: candidate.base_revision(),
            patch_count: candidate.patches().len(),
            patches: serde_json::to_value(candidate.patches())
                .map_err(|error| ReviewedChangeError::Storage(error.to_string()))?,
            digest: candidate.digest().to_owned(),
            workflow_fingerprint: candidate.workflow_fingerprint().to_owned(),
            workflow: serde_json::to_value(candidate.workflow())
                .map_err(|error| ReviewedChangeError::Storage(error.to_string()))?,
            readiness_blockers: candidate
                .readiness_blockers()
                .iter()
                .map(|blocker| ReadinessBlockerDto {
                    code: blocker.code.clone(),
                    pointer: blocker.pointer.clone(),
                    constraint: blocker.constraint.clone(),
                })
                .collect(),
            expires_at: candidate.expires_at(),
        })
    }
}

impl JsonSchema for PreparePatchInput {
    fn schema_name() -> String {
        "PreparePatchInput".to_owned()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let schema = WorkflowApplyPatchInput::json_schema(generator);
        let mut value = serde_json::to_value(schema).unwrap_or(json!(false));
        if let Some(properties) = value.get_mut("properties").and_then(Value::as_object_mut) {
            properties
                .insert("prior_candidate_id".to_owned(), json!({ "type": ["string", "null"] }));
        }
        if let Some(required) = value.get_mut("required").and_then(Value::as_array_mut) {
            required.push(json!("prior_candidate_id"));
        }
        serde_json::from_value(value).unwrap_or(Schema::Bool(false))
    }
}

fn handler_error(error: ReviewedChangeError) -> OperationHandlerError {
    OperationHandlerError::new("REVIEWED_CHANGE_FAILED", error.to_string())
}
