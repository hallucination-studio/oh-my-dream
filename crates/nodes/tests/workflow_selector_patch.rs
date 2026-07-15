use assets::AssetStore;
use engine::{
    CapabilityRef, InputBinding, NodeParams, NodeRef, NodeRegistry, PatchOutputRef, Workflow,
    WorkflowNode, WorkflowPatch, WorkflowPatchOperation, apply_workflow_patch,
};
use nodes::{
    CapabilityNodeStatus, GeneratedOutput, GenerationContext, GenerationError,
    ImageToVideoGenerator, ImageToVideoRequest, ReferenceImageGenerationRequest,
    ReferenceImageGenerator, ReferenceVideoGenerationRequest, ReferenceVideoGenerator,
    SharedAssetStore, TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator,
    TextToImageRequest,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn add_node_persists_modality_and_canonical_mode() {
    let (_directory, registry) = registry();
    let result = apply_workflow_patch(
        &registry,
        &empty_workflow(),
        &WorkflowPatch { operations: vec![add("image", "TextToImage")] },
    )
    .expect("add node should succeed");

    assert_eq!(result.workflow.nodes[0].type_id, "Image");
    assert_eq!(result.workflow.nodes[0].params["mode"], "text");
    assert_eq!(result.workflow.nodes[0].params["model"], "mock-image");
}

#[test]
fn patch_canonicalizes_a_legacy_exact_id_base_before_operations() {
    let (_directory, registry) = registry();
    let legacy = Workflow {
        version: "1.0".to_owned(),
        project_id: "project".to_owned(),
        nodes: vec![WorkflowNode {
            id: "video".to_owned(),
            type_id: "ImageToVideo".to_owned(),
            contract_version: "1.0".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    };
    let result = apply_workflow_patch(
        &registry,
        &legacy,
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::SetPosition {
                node: NodeRef::Id { id: "video".to_owned() },
                position: [12.0, 24.0],
            }],
        },
    )
    .expect("legacy base should canonicalize");

    assert_eq!(result.workflow.nodes[0].type_id, "Video");
    assert_eq!(result.workflow.nodes[0].params["mode"], "image");
    assert_eq!(result.workflow.nodes[0].params["model"], "mock-video");
}

#[test]
fn legacy_exact_id_node_can_change_mode_in_one_replace_params_operation() {
    let (_directory, registry) = registry();
    let legacy = Workflow {
        version: "1.0".to_owned(),
        project_id: "project".to_owned(),
        nodes: vec![WorkflowNode {
            id: "video".to_owned(),
            type_id: "VideoConcat".to_owned(),
            contract_version: "1.0".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    };
    let result = apply_workflow_patch(
        &registry,
        &legacy,
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::ReplaceParams {
                node: NodeRef::Id { id: "video".to_owned() },
                params: NodeParams::from_iter([
                    ("mode".to_owned(), json!("image")),
                    ("model".to_owned(), json!("mock-video")),
                ]),
            }],
        },
    )
    .expect("legacy node should canonicalize before changing mode");

    assert_eq!(result.workflow.nodes[0].type_id, "Video");
    assert_eq!(result.workflow.nodes[0].params["mode"], "image");
}

#[test]
fn replace_params_selects_a_different_exact_capability_under_one_modality() {
    let (_directory, registry) = registry();
    let initial = apply_workflow_patch(
        &registry,
        &empty_workflow(),
        &WorkflowPatch { operations: vec![add("video", "VideoConcat")] },
    )
    .expect("concat node should be added");
    let result = apply_workflow_patch(
        &registry,
        &initial.workflow,
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::ReplaceParams {
                node: NodeRef::Id { id: "n1".to_owned() },
                params: NodeParams::from_iter([
                    ("mode".to_owned(), json!("image")),
                    ("model".to_owned(), json!("mock-video")),
                ]),
            }],
        },
    )
    .expect("unwired video node should change mode");

    assert_eq!(result.workflow.nodes[0].type_id, "Video");
    assert_eq!(result.workflow.nodes[0].params["mode"], "image");
}

#[test]
fn incompatible_mode_change_rejects_without_changing_the_existing_graph() {
    let (_directory, registry) = registry();
    let initial = apply_workflow_patch(
        &registry,
        &empty_workflow(),
        &WorkflowPatch {
            operations: vec![
                add("first", "ImageToVideo"),
                add("second", "ImageToVideo"),
                add("concat", "VideoConcat"),
                WorkflowPatchOperation::SetInput {
                    node: NodeRef::Alias { alias: "concat".to_owned() },
                    input: "clips".to_owned(),
                    binding: InputBinding::ordered_many(vec![
                        PatchOutputRef {
                            node: NodeRef::Alias { alias: "first".to_owned() },
                            output: "video".to_owned(),
                        },
                        PatchOutputRef {
                            node: NodeRef::Alias { alias: "second".to_owned() },
                            output: "video".to_owned(),
                        },
                    ]),
                },
            ],
        },
    )
    .expect("wired concat graph should be valid");
    let original = initial.workflow.clone();

    let error = apply_workflow_patch(
        &registry,
        &initial.workflow,
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::ReplaceParams {
                node: NodeRef::Id { id: "n3".to_owned() },
                params: NodeParams::from_iter([
                    ("mode".to_owned(), json!("image")),
                    ("model".to_owned(), json!("mock-video")),
                ]),
            }],
        },
    )
    .expect_err("image mode does not declare the existing clips input");

    assert_eq!(error.code(), "INPUT_NOT_DECLARED");
    assert_eq!(initial.workflow, original);
}

#[test]
fn node_projection_resolves_a_canonical_selector_through_the_registry() {
    let (_directory, registry) = registry();
    let node = WorkflowNode {
        id: "video".to_owned(),
        type_id: "Video".to_owned(),
        contract_version: "1.0".to_owned(),
        params: NodeParams::from_iter([("mode".to_owned(), json!("image"))]),
        inputs: BTreeMap::new(),
        position: None,
    };

    let resolution = nodes::resolve_workflow_node(&registry, &node);

    assert_eq!(resolution.status, CapabilityNodeStatus::Ready);
    assert_eq!(resolution.node.type_id, "Video");
    assert_eq!(resolution.node.params["model"], "mock-video");
}

fn add(alias: &str, id: &str) -> WorkflowPatchOperation {
    WorkflowPatchOperation::AddNode {
        alias: alias.to_owned(),
        capability: CapabilityRef::new(id, "1.0"),
        params: NodeParams::new(),
        position: None,
    }
}

fn empty_workflow() -> Workflow {
    Workflow { version: "1.0".to_owned(), project_id: "project".to_owned(), nodes: Vec::new() }
}

fn registry() -> (TempDir, NodeRegistry) {
    let directory = TempDir::new().expect("asset root");
    let store: SharedAssetStore =
        Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let mut registry = NodeRegistry::new();
    nodes::register_all(
        &mut registry,
        nodes::GenerationAdapters::new(
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
        ),
        store,
        Arc::new(support::MissingResolver),
    )
    .expect("capability registration");
    (directory, registry)
}

struct NoopGenerator;

impl TextToImageGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: TextToImageRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!("patch tests do not execute nodes")
    }
}

impl ImageToVideoGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ImageToVideoRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!("patch tests do not execute nodes")
    }
}

impl ReferenceImageGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ReferenceImageGenerationRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!("patch tests do not execute nodes")
    }
}

impl ReferenceVideoGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ReferenceVideoGenerationRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!("patch tests do not execute nodes")
    }
}

impl TextToAudioGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: TextToAudioRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!("patch tests do not execute nodes")
    }
}
mod support;
