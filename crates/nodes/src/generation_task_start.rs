//! Node-owned durable Generation Task start boundary.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
pub use engine::node_capability::NodeCapabilityGenerationTaskStartFailure;
use engine::node_capability::{
    NodeCapabilityOutputKey, WorkflowManagedContentFingerprint, WorkflowManagedImageRef,
    WorkflowNodeExecutionContext, WorkflowNodeExecutionOrigin, WorkflowTextValue,
};
use uuid::{Uuid, Variant, Version};

use crate::GenerationProfileRef;

/// Provider-independent text-to-image aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageAspectRatio {
    /// Square 1:1.
    Square,
    /// Landscape 4:3.
    LandscapeFourByThree,
    /// Portrait 3:4.
    PortraitThreeByFour,
    /// Landscape 16:9.
    LandscapeSixteenByNine,
    /// Portrait 9:16.
    PortraitNineBySixteen,
}

/// Provider-independent image-to-video duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageToVideoDurationSeconds {
    /// Five seconds.
    Five,
    /// Ten seconds.
    Ten,
}

/// One exact immutable managed Asset snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCapabilityGenerationTaskAssetSnapshot {
    image: WorkflowManagedImageRef,
}

impl NodeCapabilityGenerationTaskAssetSnapshot {
    /// Captures one exact readable Image reference.
    #[must_use]
    pub const fn image(image: WorkflowManagedImageRef) -> Self {
        Self { image }
    }

    /// Returns the exact Image reference.
    #[must_use]
    pub const fn image_ref(self) -> WorkflowManagedImageRef {
        self.image
    }

    /// Returns the immutable content fingerprint.
    #[must_use]
    pub const fn content_fingerprint(self) -> WorkflowManagedContentFingerprint {
        self.image.content_fingerprint()
    }
}

/// Closed node-owned generation operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityGenerationTaskRequest {
    /// Text-to-image generation.
    Image {
        /// Exact structured prompt.
        prompt: WorkflowTextValue,
        /// Normalized aspect ratio.
        aspect_ratio: ImageAspectRatio,
    },
    /// Image-to-video generation.
    Video {
        /// Exact readable input Image snapshot.
        input_image: NodeCapabilityGenerationTaskAssetSnapshot,
        /// Optional exact structured prompt.
        prompt: Option<WorkflowTextValue>,
        /// Normalized duration.
        duration_seconds: ImageToVideoDurationSeconds,
    },
    /// Text-to-speech generation.
    Voice {
        /// Exact structured speech text.
        text: WorkflowTextValue,
    },
}

impl NodeCapabilityGenerationTaskRequest {
    const fn expected_output_key(&self) -> &'static str {
        match self {
            Self::Image { .. } => "image",
            Self::Video { .. } => "video",
            Self::Voice { .. } => "audio",
        }
    }

    const fn expected_capability_id(&self) -> &'static str {
        match self {
            Self::Image { .. } => "image.generate_from_text",
            Self::Video { .. } => "video.generate_from_image",
            Self::Voice { .. } => "audio.synthesize_speech_from_text",
        }
    }
}

/// Complete request crossing from a Node Capability to durable Task admission.
#[derive(Clone, Debug)]
pub struct NodeCapabilityGenerationTaskStartRequest {
    context: WorkflowNodeExecutionContext,
    origin: WorkflowNodeExecutionOrigin,
    profile_ref: GenerationProfileRef,
    request: NodeCapabilityGenerationTaskRequest,
    primary_output_key: NodeCapabilityOutputKey,
    input_assets: Vec<NodeCapabilityGenerationTaskAssetSnapshot>,
}

impl NodeCapabilityGenerationTaskStartRequest {
    /// Creates a request only when operation, output, and ordered Asset snapshots agree.
    pub fn try_new(
        context: WorkflowNodeExecutionContext,
        origin: WorkflowNodeExecutionOrigin,
        profile_ref: GenerationProfileRef,
        request: NodeCapabilityGenerationTaskRequest,
        primary_output_key: NodeCapabilityOutputKey,
        input_assets: Vec<NodeCapabilityGenerationTaskAssetSnapshot>,
    ) -> Result<Self, NodeCapabilityGenerationTaskStartFailure> {
        let assets_match = match &request {
            NodeCapabilityGenerationTaskRequest::Image { .. }
            | NodeCapabilityGenerationTaskRequest::Voice { .. } => input_assets.is_empty(),
            NodeCapabilityGenerationTaskRequest::Video { input_image, .. } => {
                input_assets.as_slice() == [*input_image]
            }
        };
        let origin_matches = origin.capability_contract_ref().id().as_str()
            == request.expected_capability_id()
            && origin.capability_contract_ref().version().major() == 1
            && origin.capability_contract_ref().version().minor() == 0;
        if primary_output_key.as_str() != request.expected_output_key()
            || !assets_match
            || !origin_matches
        {
            return Err(NodeCapabilityGenerationTaskStartFailure::InvalidRequest);
        }
        Ok(Self { context, origin, profile_ref, request, primary_output_key, input_assets })
    }

    /// Returns unchanged execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns unchanged frozen producer coordinates.
    #[must_use]
    pub const fn origin(&self) -> &WorkflowNodeExecutionOrigin {
        &self.origin
    }
    /// Returns the selected provider-independent profile.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns the closed generation operation.
    #[must_use]
    pub const fn request(&self) -> &NodeCapabilityGenerationTaskRequest {
        &self.request
    }
    /// Returns the declared primary output key.
    #[must_use]
    pub const fn primary_output_key(&self) -> &NodeCapabilityOutputKey {
        &self.primary_output_key
    }
    /// Returns ordered exact input Asset snapshots.
    #[must_use]
    pub fn input_assets(&self) -> &[NodeCapabilityGenerationTaskAssetSnapshot] {
        &self.input_assets
    }
}

/// Opaque local durable Task identity for boundary diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityGenerationTaskId(Uuid);

impl NodeCapabilityGenerationTaskId {
    /// Restores one RFC 9562 UUIDv4 Task identity.
    pub fn from_uuid(value: Uuid) -> Result<Self, NodeCapabilityGenerationTaskStartFailure> {
        if value.get_version() != Some(Version::Random) || value.get_variant() != Variant::RFC4122 {
            return Err(NodeCapabilityGenerationTaskStartFailure::InvalidRequest);
        }
        Ok(Self(value))
    }
    /// Returns the exact UUID without selecting a wire encoding.
    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// Durable Task admission result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityGenerationTaskStartResult {
    task_id: NodeCapabilityGenerationTaskId,
}

impl NodeCapabilityGenerationTaskStartResult {
    /// Records the durable local Task identity.
    #[must_use]
    pub const fn new(task_id: NodeCapabilityGenerationTaskId) -> Self {
        Self { task_id }
    }
    /// Returns the durable local Task identity.
    #[must_use]
    pub const fn task_id(&self) -> &NodeCapabilityGenerationTaskId {
        &self.task_id
    }
}

/// Starts one durable Generation Task for a Node Capability.
#[async_trait]
pub trait NodeCapabilityGenerationTaskStarterInterface: Send + Sync {
    /// Returns only after Task state and its initial submit effect are durable.
    async fn start_generation_task(
        &self,
        request: NodeCapabilityGenerationTaskStartRequest,
    ) -> Result<NodeCapabilityGenerationTaskStartResult, NodeCapabilityGenerationTaskStartFailure>;
}

#[async_trait]
impl<T: NodeCapabilityGenerationTaskStarterInterface + ?Sized>
    NodeCapabilityGenerationTaskStarterInterface for Arc<T>
{
    async fn start_generation_task(
        &self,
        request: NodeCapabilityGenerationTaskStartRequest,
    ) -> Result<NodeCapabilityGenerationTaskStartResult, NodeCapabilityGenerationTaskStartFailure>
    {
        self.as_ref().start_generation_task(request).await
    }
}

/// Deterministic observable Task starter for capability tests.
#[derive(Clone)]
pub struct NodeCapabilityGenerationTaskStarterFakeImpl {
    state: Arc<Mutex<StarterFakeState>>,
}

struct StarterFakeState {
    failure: Option<NodeCapabilityGenerationTaskStartFailure>,
    requests: Vec<NodeCapabilityGenerationTaskStartRequest>,
    accepted:
        Vec<(NodeCapabilityGenerationTaskStartRequest, NodeCapabilityGenerationTaskStartResult)>,
}

impl Default for NodeCapabilityGenerationTaskStarterFakeImpl {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(StarterFakeState {
                failure: None,
                requests: Vec::new(),
                accepted: Vec::new(),
            })),
        }
    }
}

impl NodeCapabilityGenerationTaskStarterFakeImpl {
    /// Creates a fake that returns one exact failure.
    #[must_use]
    pub fn failing(failure: NodeCapabilityGenerationTaskStartFailure) -> Self {
        Self {
            state: Arc::new(Mutex::new(StarterFakeState {
                failure: Some(failure),
                requests: Vec::new(),
                accepted: Vec::new(),
            })),
        }
    }
    /// Returns all observed requests in call order.
    pub fn requests(&self) -> Vec<NodeCapabilityGenerationTaskStartRequest> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).requests.clone()
    }
}

#[async_trait]
impl NodeCapabilityGenerationTaskStarterInterface for NodeCapabilityGenerationTaskStarterFakeImpl {
    async fn start_generation_task(
        &self,
        request: NodeCapabilityGenerationTaskStartRequest,
    ) -> Result<NodeCapabilityGenerationTaskStartResult, NodeCapabilityGenerationTaskStartFailure>
    {
        if request.context.cancellation.is_cancelled() {
            return Err(NodeCapabilityGenerationTaskStartFailure::Cancelled);
        }
        if request.context.deadline.is_reached_at(std::time::Instant::now()) {
            return Err(NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded);
        }
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.requests.push(request.clone());
        if let Some(failure) = state.failure {
            return Err(failure);
        }
        if let Some((existing, result)) = state.accepted.iter().find(|(existing, _)| {
            existing.context.project_id == request.context.project_id
                && existing.context.node_execution_id == request.context.node_execution_id
        }) {
            return if start_requests_match(existing, &request) {
                Ok(result.clone())
            } else {
                Err(NodeCapabilityGenerationTaskStartFailure::Conflict)
            };
        }
        let task_id =
            NodeCapabilityGenerationTaskId::from_uuid(request.context.node_execution_id.as_uuid())?;
        let result = NodeCapabilityGenerationTaskStartResult::new(task_id);
        state.accepted.push((request, result.clone()));
        Ok(result)
    }
}

fn start_requests_match(
    left: &NodeCapabilityGenerationTaskStartRequest,
    right: &NodeCapabilityGenerationTaskStartRequest,
) -> bool {
    left.context.project_id == right.context.project_id
        && left.context.workflow_run_id == right.context.workflow_run_id
        && left.context.node_execution_id == right.context.node_execution_id
        && left.origin == right.origin
        && left.profile_ref == right.profile_ref
        && left.request == right.request
        && left.primary_output_key == right.primary_output_key
        && left.input_assets == right.input_assets
}
