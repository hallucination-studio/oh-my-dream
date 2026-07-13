use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

mod contract;
mod dispatch;
mod schema_policy;

pub use contract::OperationContract;
use dispatch::{ErasedOperationHandler, TypedOperationHandler};
pub use dispatch::{
    OperationDispatchError, OperationHandler, OperationHandlerError, OperationSchemaViolation,
};
use schema_policy::operation_schemas;
pub use schema_policy::{OperationInputSchemaMode, OperationRegistrationError};
/// The durable category of effect produced by an assistant operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationEffect {
    /// Reads local state without changing it.
    LocalRead,
    /// Applies a user-visible Workflow change that can be reversed.
    VisibleReversibleWorkflowPatch,
    /// Executes an effect prepared earlier and authorized by trusted context.
    PreparedApprovalExecution,
}

/// Trusted proof that a prepared operation was approved outside model input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedEffect {
    operation_id: String,
    operation_version: u32,
    approval_id: String,
}

impl ApprovedEffect {
    /// Creates trusted approval proof for one exact operation version.
    #[must_use]
    pub fn new(
        operation_id: impl Into<String>,
        operation_version: u32,
        approval_id: impl Into<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            operation_version,
            approval_id: approval_id.into(),
        }
    }

    /// Returns the approved operation ID.
    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    /// Returns the approved operation version.
    #[must_use]
    pub fn operation_version(&self) -> u32 {
        self.operation_version
    }
    /// Returns the trusted approval identifier.
    #[must_use]
    pub fn approval_id(&self) -> &str {
        &self.approval_id
    }
}

/// Trusted invocation data supplied by the Rust application boundary.
///
/// This type intentionally has no Serde or JSON Schema implementation, so it
/// cannot become part of model-controlled operation arguments by derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    project_id: String,
    session_id: String,
    request_id: String,
    tool_version: u32,
    approved_effect: Option<ApprovedEffect>,
}

impl RequestContext {
    /// Creates trusted context for one operation invocation.
    #[must_use]
    pub fn new(
        project_id: impl Into<String>,
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        tool_version: u32,
        approved_effect: Option<ApprovedEffect>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: session_id.into(),
            request_id: request_id.into(),
            tool_version,
            approved_effect,
        }
    }

    /// Returns the trusted project identifier.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    /// Returns the trusted assistant session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    /// Returns the trusted request identifier.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    /// Returns the exact operation contract version requested by the caller.
    #[must_use]
    pub fn tool_version(&self) -> u32 {
        self.tool_version
    }
    /// Returns trusted approval proof when the invocation was approved.
    #[must_use]
    pub fn approved_effect(&self) -> Option<&ApprovedEffect> {
        self.approved_effect.as_ref()
    }
}

/// One Rust-owned assistant operation contract and its bound handler.
pub struct OperationRegistration {
    id: String,
    version: u32,
    description: String,
    effect: OperationEffect,
    input_schema_mode: OperationInputSchemaMode,
    input_schema: Value,
    output_schema: Value,
    handler: Box<dyn ErasedOperationHandler>,
}

impl OperationRegistration {
    /// Registers canonical DTOs and a handler under stable operation metadata.
    pub fn new<I, O, H>(
        id: impl Into<String>,
        version: u32,
        description: impl Into<String>,
        effect: OperationEffect,
        input_schema_mode: OperationInputSchemaMode,
        handler: H,
    ) -> Result<Self, OperationRegistrationError>
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + Send + 'static,
        H: OperationHandler<I, O> + 'static,
    {
        let (input_schema, output_schema) = operation_schemas::<I, O>(input_schema_mode)?;
        let handler = TypedOperationHandler::<I, O, H>::new(handler, &input_schema)
            .map_err(|message| OperationRegistrationError::ValidatorCompilation { message })?;
        Ok(Self {
            id: id.into(),
            version,
            description: description.into(),
            effect,
            input_schema_mode,
            input_schema,
            output_schema,
            handler: Box::new(handler),
        })
    }

    /// Returns the stable operation identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Returns the operation contract version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
    /// Returns the model-facing operation description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Returns the operation effect classification.
    #[must_use]
    pub fn effect(&self) -> OperationEffect {
        self.effect
    }
    /// Returns the explicit canonical input schema policy.
    #[must_use]
    pub fn input_schema_mode(&self) -> OperationInputSchemaMode {
        self.input_schema_mode
    }
    /// Returns the SDK strictness derived directly from the input policy.
    #[must_use]
    pub fn sdk_strict_json_schema(&self) -> bool {
        self.input_schema_mode.sdk_strict_json_schema()
    }
    /// Returns the generated canonical input JSON Schema.
    #[must_use]
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }
    /// Returns the generated canonical output JSON Schema.
    #[must_use]
    pub fn output_schema(&self) -> &Value {
        &self.output_schema
    }
    /// Projects model-facing metadata and schemas without the bound handler.
    #[must_use]
    pub fn contract(&self) -> OperationContract {
        OperationContract::from_registration(self)
    }
    /// Validates model JSON, invokes the bound handler, and returns output JSON.
    pub async fn dispatch(
        &self,
        context: &RequestContext,
        input: Value,
    ) -> Result<Value, OperationDispatchError> {
        self.validate_context(context)?;
        self.handler.dispatch(&self.id, context, input).await
    }
    fn validate_context(&self, context: &RequestContext) -> Result<(), OperationDispatchError> {
        if context.tool_version() != self.version {
            return Err(OperationDispatchError::ToolVersionMismatch {
                operation_id: self.id.clone(),
                registered_version: self.version,
                context_version: context.tool_version(),
            });
        }
        if self.effect != OperationEffect::PreparedApprovalExecution {
            return Ok(());
        }
        let approved =
            context.approved_effect().ok_or_else(|| OperationDispatchError::ApprovalRequired {
                operation_id: self.id.clone(),
                operation_version: self.version,
            })?;
        if approved.operation_id() == self.id && approved.operation_version() == self.version {
            return Ok(());
        }
        Err(OperationDispatchError::ApprovalMismatch {
            operation_id: self.id.clone(),
            operation_version: self.version,
            approved_operation_id: approved.operation_id().to_owned(),
            approved_operation_version: approved.operation_version(),
        })
    }
}
