//! Workflow graph model and its serialized (JSON) form.
//!
//! The on-disk format follows docs/DESIGN.md §5: named ports, `inputs`
//! referencing `[source_node_id, source_output_name]`, a `params` object, and a
//! UI-only `position`. Logic and layout are kept separate — `position` never
//! affects execution.

use serde::{Deserialize, Serialize};

use crate::DEFAULT_CAPABILITY_VERSION;
use crate::registry::NodeParams;

/// A reference to a specific output of an upstream node: `[node_id, output]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputRef(pub String, pub String);

impl OutputRef {
    /// The upstream node id.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.0
    }

    /// The named output on the upstream node.
    #[must_use]
    pub fn output_name(&self) -> &str {
        &self.1
    }
}

/// A single node entry in the serialized workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    /// Unique node id within the workflow.
    pub id: String,
    /// The node type, resolved against the registry.
    #[serde(rename = "type")]
    pub type_id: String,
    /// Exact capability contract version used to construct this node.
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    /// Constructor parameters / widget values for this node.
    #[serde(default)]
    pub params: NodeParams,
    /// Wiring: input port name -> upstream `[node_id, output_name]`.
    #[serde(default)]
    pub inputs: std::collections::BTreeMap<String, OutputRef>,
    /// UI layout only; ignored by execution.
    #[serde(default)]
    pub position: Option<[f64; 2]>,
}

/// A whole workflow as stored on disk or sent from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Format version, for forward compatibility.
    pub version: String,
    /// Project this workflow belongs to.
    #[serde(default = "default_project_id")]
    pub project_id: String,
    /// The nodes making up the graph.
    pub nodes: Vec<WorkflowNode>,
}

fn default_project_id() -> String {
    "default".to_owned()
}

fn default_contract_version() -> String {
    DEFAULT_CAPABILITY_VERSION.to_owned()
}

#[cfg(test)]
mod tests {
    use super::WorkflowNode;
    use serde_json::json;

    #[test]
    fn workflow_nodes_persist_exact_contract_versions() {
        let node = WorkflowNode {
            id: "prompt".to_owned(),
            type_id: "TextPrompt".to_owned(),
            contract_version: "1.0".to_owned(),
            params: serde_json::Map::new(),
            inputs: std::collections::BTreeMap::new(),
            position: None,
        };
        assert_eq!(
            serde_json::to_value(node).expect("workflow node JSON"),
            json!({
                "id": "prompt",
                "type": "TextPrompt",
                "contract_version": "1.0",
                "params": {},
                "inputs": {},
                "position": null
            })
        );
    }

    #[test]
    fn legacy_nodes_default_to_the_first_contract_version_when_read() {
        let node: WorkflowNode = serde_json::from_value(json!({
            "id": "prompt",
            "type": "TextPrompt",
            "params": {},
            "inputs": {}
        }))
        .expect("legacy workflow node");
        assert_eq!(node.contract_version, "1.0");
    }
}
