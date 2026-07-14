use std::process::ExitStatus;
use thiserror::Error;

use crate::assistant_operations::OperationDispatchError;
use crate::assistant_sidecar::AssistantStdioSidecarError;
use crate::assistant_transport::{AssistantFrameKind, AssistantTransportError};

/// Failure while orchestrating one assistant sidecar invocation.
#[derive(Debug, Error)]
pub enum AssistantRuntimeError {
    #[error("assistant runtime limits are invalid: {message}")]
    InvalidLimits { message: &'static str },
    #[error("duplicate assistant operation registration: {operation_id}")]
    DuplicateOperation { operation_id: String },
    #[error("assistant session path is not valid UTF-8")]
    InvalidSessionPath,
    #[error("resume invocation input must be null")]
    ResumeInputMustBeNull,
    #[error(transparent)]
    Sidecar(#[from] AssistantStdioSidecarError),
    #[error(transparent)]
    Transport(#[from] AssistantTransportError),
    #[error("invalid {kind:?} payload: {message}")]
    InvalidPayload { kind: AssistantFrameKind, message: String },
    #[error("assistant frame invocation mismatch: expected {expected}, received {actual}")]
    InvocationMismatch { expected: String, actual: String },
    #[error("assistant snapshot session does not match the invocation")]
    SessionMismatch,
    #[error("assistant snapshot status is invalid: {status}")]
    InvalidSnapshotStatus { status: String },
    #[error("assistant frame violates runtime state ordering: {event}")]
    InvalidStateTransition { event: &'static str },
    #[error("unknown assistant operation: {operation_id}")]
    UnknownOperation { operation_id: String },
    #[error("operation {operation_id} arguments_json is invalid: {source}")]
    InvalidArguments {
        operation_id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("assistant operation dispatch failed: {0}")]
    Operation(#[from] OperationDispatchError),
    #[error("operation {operation_id} output serialization failed: {source}")]
    OutputSerialization {
        operation_id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("unexpected assistant frame: {kind:?}")]
    UnexpectedFrame { kind: AssistantFrameKind },
    #[error("assistant completed without a session snapshot")]
    MissingSnapshot,
    #[error("approval request completed without its waiting snapshot")]
    MissingApprovalSnapshot,
    #[error("approval request and snapshot carried different opaque state")]
    ApprovalStateMismatch,
    #[error("approval identity does not match the trusted pending call")]
    ApprovalMismatch,
    #[error("approval scope does not match the interrupted invocation")]
    ApprovalScopeMismatch,
    #[error("approved effect was requested more than once")]
    ApprovalReuse,
    #[error("a rejected approval attempted to execute an effect")]
    RejectedApprovalExecution,
    #[error("assistant sidecar error {code}: {message}")]
    SidecarReported { code: String, message: String },
    #[error("assistant invocation exceeded its deadline")]
    InvocationTimeout,
    #[error("assistant sidecar shutdown exceeded its deadline")]
    ShutdownTimeout,
    #[error("assistant invocation exceeded {resource} budget {maximum}")]
    ResourceLimit { resource: &'static str, maximum: usize },
    #[error("assistant sidecar exited unsuccessfully: {status}")]
    ProcessExit { status: ExitStatus },
    #[error("assistant event sink failed: {message}")]
    EventSink { message: String },
}
