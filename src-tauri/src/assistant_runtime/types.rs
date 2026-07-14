use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bounded resource limits for one sidecar invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssistantRuntimeLimits {
    pub(super) invocation_timeout: Duration,
    pub(super) shutdown_timeout: Duration,
    pub(super) max_incoming_frames: usize,
    pub(super) max_collected_bytes: usize,
}

/// Consumer-owned sink for native Responses event objects.
pub trait AssistantEventSink: Send {
    /// Delivers one validated Responses event without remapping its fields.
    fn emit(&mut self, event: Value) -> Result<(), super::AssistantRuntimeError>;
}

pub(super) struct NoopEventSink;

impl AssistantEventSink for NoopEventSink {
    fn emit(&mut self, _event: Value) -> Result<(), super::AssistantRuntimeError> {
        Ok(())
    }
}

impl AssistantRuntimeLimits {
    /// Creates non-zero invocation limits.
    pub fn new(
        invocation_timeout: Duration,
        shutdown_timeout: Duration,
        max_incoming_frames: usize,
        max_collected_bytes: usize,
    ) -> Result<Self, super::AssistantRuntimeError> {
        if invocation_timeout.is_zero() || shutdown_timeout.is_zero() {
            return Err(super::AssistantRuntimeError::InvalidLimits {
                message: "timeouts must be non-zero",
            });
        }
        if max_incoming_frames == 0 || max_collected_bytes == 0 {
            return Err(super::AssistantRuntimeError::InvalidLimits {
                message: "budgets must be non-zero",
            });
        }
        Ok(Self { invocation_timeout, shutdown_timeout, max_incoming_frames, max_collected_bytes })
    }
}

impl Default for AssistantRuntimeLimits {
    fn default() -> Self {
        Self {
            invocation_timeout: Duration::from_secs(300),
            shutdown_timeout: Duration::from_secs(5),
            max_incoming_frames: 512,
            max_collected_bytes: 8 * 1_048_576,
        }
    }
}

/// Trusted invocation identifiers that never enter model-controlled arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedInvocationContext {
    pub(super) project_id: String,
    pub(super) request_id: String,
    pub(super) selected_node_ids: Vec<String>,
    pub(super) selected_asset_ids: Vec<String>,
}

impl TrustedInvocationContext {
    /// Creates trusted project and request scope for one invocation.
    pub fn new(project_id: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            request_id: request_id.into(),
            selected_node_ids: Vec::new(),
            selected_asset_ids: Vec::new(),
        }
    }

    /// Adds the trusted UI selection for this Project-scoped invocation.
    #[must_use]
    pub fn with_selection(
        mut self,
        selected_node_ids: Vec<String>,
        selected_asset_ids: Vec<String>,
    ) -> Self {
        self.selected_node_ids = selected_node_ids;
        self.selected_asset_ids = selected_asset_ids;
        self
    }

    /// Returns the trusted Project identifier.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Returns the Rust-generated request identifier.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Returns the trusted selected node pointers.
    #[must_use]
    pub fn selected_node_ids(&self) -> &[String] {
        &self.selected_node_ids
    }

    /// Returns the trusted selected Asset pointers.
    #[must_use]
    pub fn selected_asset_ids(&self) -> &[String] {
        &self.selected_asset_ids
    }
}

/// Application-owned request to start one assistant invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantInvocation {
    pub(super) invocation_id: String,
    pub(super) session_id: String,
    pub(super) session_path: PathBuf,
    pub(super) input: Option<String>,
}

impl AssistantInvocation {
    /// Creates a new invocation or a resume request with null input.
    pub fn new(
        invocation_id: impl Into<String>,
        session_id: impl Into<String>,
        session_path: impl AsRef<Path>,
        input: Option<String>,
    ) -> Self {
        Self {
            invocation_id: invocation_id.into(),
            session_id: session_id.into(),
            session_path: session_path.as_ref().to_owned(),
            input,
        }
    }

    /// Returns the Rust-generated invocation correlation identifier.
    #[must_use]
    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    /// Returns the Project-scoped SDK session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the exact user text passed to the SDK, when this is a new turn.
    #[must_use]
    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }

    /// Returns the file-backed SDK session path.
    #[must_use]
    pub fn session_path(&self) -> &Path {
        &self.session_path
    }
}

/// Opaque sidecar snapshot returned at an invocation boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct AssistantSessionSnapshot {
    pub(super) session_id: String,
    pub(super) status: String,
    pub(super) state: Value,
}

impl AssistantSessionSnapshot {
    /// Returns the SDK session identifier.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    /// Returns the terminal snapshot status.
    pub fn status(&self) -> &str {
        &self.status
    }
    /// Returns the opaque sidecar state value.
    pub fn state(&self) -> &Value {
        &self.state
    }
}

/// Exact pending effect identity carried across sidecar restart.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssistantPendingApproval {
    pub(super) call_id: String,
    pub(super) operation_id: String,
    pub(super) operation_version: u32,
    pub(super) arguments_json: String,
}

impl AssistantPendingApproval {
    /// Returns the SDK tool call identifier.
    pub fn call_id(&self) -> &str {
        &self.call_id
    }
    /// Returns the registered operation identifier.
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    /// Returns the trusted registered operation version.
    pub fn operation_version(&self) -> u32 {
        self.operation_version
    }
    /// Returns the exact SDK argument JSON approved by the user.
    pub fn arguments_json(&self) -> &str {
        &self.arguments_json
    }
}

/// Suspended assistant result containing only opaque SDK state and effect identity.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct AssistantWaitingApproval {
    pub(super) state: Value,
    pub(super) pending: AssistantPendingApproval,
    pub(super) project_id: String,
    pub(super) session_id: String,
    pub(super) session_path: PathBuf,
}

impl AssistantWaitingApproval {
    /// Returns the opaque SDK state required to resume the interrupted run.
    pub fn state(&self) -> &Value {
        &self.state
    }
    /// Returns the exact pending operation identity.
    pub fn pending(&self) -> &AssistantPendingApproval {
        &self.pending
    }

    /// Returns the trusted Project scope of the pending invocation.
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Returns the trusted assistant session scope.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the file-backed SDK session path.
    pub fn session_path(&self) -> &std::path::Path {
        &self.session_path
    }
}

/// Evidence for one Rust-dispatched operation call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationCallEvidence {
    pub(super) operation_id: String,
    pub(super) operation_version: u32,
    pub(super) call_id: String,
    pub(super) arguments_json: String,
    pub(super) output_json: String,
}

impl OperationCallEvidence {
    /// Returns the dispatched operation identifier.
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    /// Returns the trusted dispatched operation version.
    pub fn operation_version(&self) -> u32 {
        self.operation_version
    }
    /// Returns the SDK tool call identifier.
    pub fn call_id(&self) -> &str {
        &self.call_id
    }
    /// Returns the exact SDK argument JSON received by Rust.
    pub fn arguments_json(&self) -> &str {
        &self.arguments_json
    }
    /// Returns the canonical output JSON emitted by Rust.
    pub fn output_json(&self) -> &str {
        &self.output_json
    }
}

/// Successful terminal assistant result.
#[derive(Clone, Debug, PartialEq)]
pub struct AssistantCompleted {
    pub(super) final_output: Value,
    pub(super) snapshot: AssistantSessionSnapshot,
    pub(super) operation_calls: Vec<OperationCallEvidence>,
}

impl AssistantCompleted {
    /// Returns the SDK final output value.
    pub fn final_output(&self) -> &Value {
        &self.final_output
    }
    /// Returns the terminal session snapshot.
    pub fn snapshot(&self) -> &AssistantSessionSnapshot {
        &self.snapshot
    }
    /// Returns Rust-dispatched operation evidence in call order.
    pub fn operation_calls(&self) -> &[OperationCallEvidence] {
        &self.operation_calls
    }
}

/// Structured terminal result of an assistant runtime invocation.
#[derive(Debug, PartialEq)]
pub enum AssistantRuntimeOutcome {
    Completed(AssistantCompleted),
    WaitingApproval(AssistantWaitingApproval),
}
