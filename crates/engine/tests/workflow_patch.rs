use engine::{
    InputBinding, InputPort, NodeInputs, NodeInterface, NodeParams, NodeRef, NodeRegistry,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, OutputRef, PatchOutputRef,
    PortCardinality, PortType, Workflow, WorkflowNode, WorkflowPatch, WorkflowPatchOperation,
    apply_workflow_patch,
};
use std::collections::BTreeMap;

#[test]
fn set_input_persists_the_requested_non_first_output() {
    let registry = patch_registry();
    let result = apply_workflow_patch(
        &registry,
        &patch_workflow(),
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::SetInput {
                node: NodeRef::Id { id: "target".to_owned() },
                input: "text".to_owned(),
                binding: InputBinding::single(PatchOutputRef {
                    node: NodeRef::Id { id: "source".to_owned() },
                    output: "second".to_owned(),
                }),
            }],
        },
    )
    .expect("named non-first output should be accepted");

    assert_eq!(
        result.workflow.nodes[1].inputs["text"],
        InputBinding::single(OutputRef("source".to_owned(), "second".to_owned()))
    );
}

#[test]
fn set_input_reports_the_exact_missing_output() {
    let registry = patch_registry();
    let error = apply_workflow_patch(
        &registry,
        &patch_workflow(),
        &WorkflowPatch {
            operations: vec![WorkflowPatchOperation::SetInput {
                node: NodeRef::Id { id: "target".to_owned() },
                input: "text".to_owned(),
                binding: InputBinding::single(PatchOutputRef {
                    node: NodeRef::Id { id: "source".to_owned() },
                    output: "missing".to_owned(),
                }),
            }],
        },
    )
    .expect_err("undeclared output should fail");

    assert_eq!(error.code(), "OUTPUT_NOT_DECLARED");
    assert_eq!(error.pointer(), "/operations/0/binding/source/output");
    assert_eq!(error.operation_index(), Some(0));
}

fn patch_registry() -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    registry.register(
        "MultiOutput",
        Box::new(|_| {
            Ok(Box::new(MultiOutputNodeImpl {
                outputs: vec![
                    OutputPort { name: "first".to_owned(), port_type: PortType::String },
                    OutputPort { name: "second".to_owned(), port_type: PortType::String },
                ],
            }))
        }),
    );
    registry.register(
        "TextTarget",
        Box::new(|_| {
            Ok(Box::new(TextTargetNodeImpl {
                inputs: vec![InputPort {
                    name: "text".to_owned(),
                    port_type: PortType::String,
                    cardinality: PortCardinality::One,
                    required: true,
                    default: None,
                }],
            }))
        }),
    );
    registry
}

fn patch_workflow() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "project".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "source".to_owned(),
                type_id: "MultiOutput".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "target".to_owned(),
                type_id: "TextTarget".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
        ],
    }
}

struct MultiOutputNodeImpl {
    outputs: Vec<OutputPort>,
}

impl NodeInterface for MultiOutputNodeImpl {
    fn type_id(&self) -> &str {
        "MultiOutput"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl<'_>,
    ) -> Result<NodeRunResult, NodeRunError> {
        unreachable!("patch tests do not execute nodes")
    }
}

struct TextTargetNodeImpl {
    inputs: Vec<InputPort>,
}

impl NodeInterface for TextTargetNodeImpl {
    fn type_id(&self) -> &str {
        "TextTarget"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &[]
    }

    fn run(
        &self,
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl<'_>,
    ) -> Result<NodeRunResult, NodeRunError> {
        unreachable!("patch tests do not execute nodes")
    }
}
