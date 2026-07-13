use std::{future::Future, marker::PhantomData};

use async_trait::async_trait;
use jsonschema::{Draft, Validator};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;

use super::RequestContext;

/// A capability-scoped asynchronous handler implemented by each operation.
#[async_trait]
pub trait OperationHandler<I, O>: Send + Sync {
    /// Executes typed model input with trusted context supplied separately.
    async fn handle(&self, context: &RequestContext, input: I) -> Result<O, OperationHandlerError>;
}

#[async_trait]
impl<I, O, F, Fut> OperationHandler<I, O> for F
where
    I: Send + 'static,
    O: Send + 'static,
    F: Fn(&RequestContext, I) -> Fut + Send + Sync,
    Fut: Future<Output = Result<O, OperationHandlerError>> + Send,
{
    async fn handle(&self, context: &RequestContext, input: I) -> Result<O, OperationHandlerError> {
        self(context, input).await
    }
}

/// A stable error returned by a concrete operation handler.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{code}: {message}")]
pub struct OperationHandlerError {
    code: String,
    message: String,
}

impl OperationHandlerError {
    /// Creates a handler error with a machine-readable code and safe message.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    /// Returns the machine-readable error code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the boundary-safe error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Failure while validating or executing a registered operation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OperationDispatchError {
    /// Trusted context requested a different operation contract version.
    #[error("operation {operation_id} version mismatch")]
    ToolVersionMismatch {
        /// Stable operation identifier.
        operation_id: String,
        /// Version owned by the registration.
        registered_version: u32,
        /// Version supplied by trusted invocation context.
        context_version: u32,
    },
    /// Prepared execution was invoked without trusted approval proof.
    #[error("operation {operation_id} version {operation_version} requires approval")]
    ApprovalRequired {
        /// Stable operation identifier.
        operation_id: String,
        /// Version owned by the registration.
        operation_version: u32,
    },
    /// Trusted approval proof targeted a different operation contract.
    #[error("operation {operation_id} approval mismatch")]
    ApprovalMismatch {
        /// Stable operation identifier.
        operation_id: String,
        /// Version owned by the registration.
        operation_version: u32,
        /// Operation identifier carried by trusted approval proof.
        approved_operation_id: String,
        /// Operation version carried by trusted approval proof.
        approved_operation_version: u32,
    },
    /// Model-controlled JSON did not satisfy the canonical input schema.
    #[error("operation {operation_id} failed schema validation")]
    SchemaValidation {
        /// Stable operation identifier.
        operation_id: String,
        /// Structured validation failures.
        violations: Vec<OperationSchemaViolation>,
    },
    /// Model-controlled JSON did not deserialize to the canonical input DTO.
    #[error("operation {operation_id} received invalid input: {message}")]
    InvalidInput {
        /// Stable operation identifier.
        operation_id: String,
        /// Serde validation detail.
        message: String,
    },
    /// The typed operation handler rejected the invocation.
    #[error("operation {operation_id} failed: {source}")]
    Handler {
        /// Stable operation identifier.
        operation_id: String,
        /// Structured handler failure.
        #[source]
        source: OperationHandlerError,
    },
    /// The canonical output DTO could not be represented as JSON.
    #[error("operation {operation_id} produced invalid output: {message}")]
    InvalidOutput {
        /// Stable operation identifier.
        operation_id: String,
        /// Serde serialization detail.
        message: String,
    },
}

/// One canonical JSON Schema validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationSchemaViolation {
    /// JSON Pointer to the invalid model-controlled value.
    pub instance_path: String,
    /// JSON Pointer to the failed schema keyword.
    pub schema_path: String,
    /// Boundary-safe validator message.
    pub message: String,
}

#[async_trait]
pub(super) trait ErasedOperationHandler: Send + Sync {
    async fn dispatch(
        &self,
        operation_id: &str,
        context: &RequestContext,
        input: Value,
    ) -> Result<Value, OperationDispatchError>;
}

pub(super) struct TypedOperationHandler<I, O, H> {
    handler: H,
    validator: Validator,
    marker: PhantomData<fn(I) -> O>,
}

impl<I, O, H> TypedOperationHandler<I, O, H> {
    pub(super) fn new(handler: H, input_schema: &Value) -> Result<Self, String> {
        let validator = jsonschema::options()
            .with_draft(Draft::Draft7)
            .build(input_schema)
            .map_err(|error| error.to_string())?;
        Ok(Self { handler, validator, marker: PhantomData })
    }
}

#[async_trait]
impl<I, O, H> ErasedOperationHandler for TypedOperationHandler<I, O, H>
where
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
    H: OperationHandler<I, O> + 'static,
{
    async fn dispatch(
        &self,
        operation_id: &str,
        context: &RequestContext,
        input: Value,
    ) -> Result<Value, OperationDispatchError> {
        let violations = self
            .validator
            .iter_errors(&input)
            .map(|error| OperationSchemaViolation {
                instance_path: error.instance_path().to_string(),
                schema_path: error.schema_path().to_string(),
                message: error.to_string(),
            })
            .collect::<Vec<_>>();
        if !violations.is_empty() {
            return Err(OperationDispatchError::SchemaValidation {
                operation_id: operation_id.to_owned(),
                violations,
            });
        }
        let typed_input = serde_json::from_value(input).map_err(|error| {
            OperationDispatchError::InvalidInput {
                operation_id: operation_id.to_owned(),
                message: error.to_string(),
            }
        })?;
        let output = self.handler.handle(context, typed_input).await.map_err(|source| {
            OperationDispatchError::Handler { operation_id: operation_id.to_owned(), source }
        })?;
        serde_json::to_value(output).map_err(|error| OperationDispatchError::InvalidOutput {
            operation_id: operation_id.to_owned(),
            message: error.to_string(),
        })
    }
}
