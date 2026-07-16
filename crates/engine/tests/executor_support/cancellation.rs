use engine::{
    CancellationSignalInterface, InputPort, NodeInputs, NodeInterface, NodeParams, NodeRegistry,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, PortType, Workflow, WorkflowNode,
    WorkflowNodeValue,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
pub(crate) struct TestCancellationImpl {
    cancelled: AtomicBool,
}

impl TestCancellationImpl {
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl CancellationSignalInterface for TestCancellationImpl {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub(crate) fn commit_then_cancel_registry(
    cancellation: Arc<TestCancellationImpl>,
    runs: Arc<AtomicUsize>,
) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    registry.register(
        "CommitThenCancel",
        Box::new(move |_| {
            Ok(Box::new(CommitThenCancelNodeImpl {
                cancellation: Arc::clone(&cancellation),
                runs: Arc::clone(&runs),
            }))
        }),
    );
    registry
}

pub(crate) fn fail_then_cancel_registry(cancellation: Arc<TestCancellationImpl>) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    registry.register(
        "FailThenCancel",
        Box::new(move |_| {
            Ok(Box::new(FailThenCancelNodeImpl { cancellation: Arc::clone(&cancellation) }))
        }),
    );
    registry
}

pub(crate) fn single_node_workflow(type_id: &str) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![WorkflowNode {
            id: "commit".to_owned(),
            type_id: type_id.to_owned(),
            contract_version: "1.0".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    }
}

struct CommitThenCancelNodeImpl {
    cancellation: Arc<TestCancellationImpl>,
    runs: Arc<AtomicUsize>,
}

impl NodeInterface for CommitThenCancelNodeImpl {
    fn type_id(&self) -> &str {
        "CommitThenCancel"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        static OUTPUTS: std::sync::LazyLock<Vec<OutputPort>> = std::sync::LazyLock::new(|| {
            vec![OutputPort { name: "text".to_owned(), port_type: PortType::String }]
        });
        &OUTPUTS
    }

    fn run(
        &self,
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.cancellation.cancel();
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            WorkflowNodeValue::String("committed".to_owned()),
        )])))
    }
}

struct FailThenCancelNodeImpl {
    cancellation: Arc<TestCancellationImpl>,
}

impl NodeInterface for FailThenCancelNodeImpl {
    fn type_id(&self) -> &str {
        "FailThenCancel"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &[]
    }

    fn run(
        &self,
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.cancellation.cancel();
        Err(Box::new(std::io::Error::other("provider cancellation failed")))
    }
}
