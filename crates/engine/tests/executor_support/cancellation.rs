use engine::{
    CancellationSignal, InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError,
    NodeRunResult, OutputPort, PortType, Value, ValueMap, Workflow, WorkflowNode,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
pub(crate) struct TestCancellation {
    cancelled: AtomicBool,
}

impl TestCancellation {
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl CancellationSignal for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub(crate) fn commit_then_cancel_registry(
    cancellation: Arc<TestCancellation>,
    runs: Arc<AtomicUsize>,
) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    registry.register(
        "CommitThenCancel",
        Box::new(move |_| {
            Ok(Box::new(CommitThenCancelNode {
                cancellation: Arc::clone(&cancellation),
                runs: Arc::clone(&runs),
            }))
        }),
    );
    registry
}

pub(crate) fn fail_then_cancel_registry(cancellation: Arc<TestCancellation>) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    registry.register(
        "FailThenCancel",
        Box::new(move |_| {
            Ok(Box::new(FailThenCancelNode { cancellation: Arc::clone(&cancellation) }))
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
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    }
}

struct CommitThenCancelNode {
    cancellation: Arc<TestCancellation>,
    runs: Arc<AtomicUsize>,
}

impl Node for CommitThenCancelNode {
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
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.cancellation.cancel();
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            Value::String("committed".to_owned()),
        )])))
    }
}

struct FailThenCancelNode {
    cancellation: Arc<TestCancellation>,
}

impl Node for FailThenCancelNode {
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
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.cancellation.cancel();
        Err(Box::new(std::io::Error::other("provider cancellation failed")))
    }
}
