//! Workflow graph model and its serialized (JSON) form.
//!
//! The on-disk format follows docs/DESIGN.md §5: named ports, `inputs`
//! referencing `[source_node_id, source_output_name]`, a `params` object, and a
//! UI-only `position`. Logic and layout are kept separate — `position` never
//! affects execution.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};

use crate::DEFAULT_CAPABILITY_VERSION;
use crate::registry::NodeParams;

/// A reference to a specific named output of an upstream node.
///
/// The canonical wire form is `{ "node_id": "...", "output": "..." }`.
/// Legacy tuple arrays remain readable for old Workflow documents.
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Serialize for OutputRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            node_id: &'a str,
            output: &'a str,
        }
        Wire { node_id: &self.0, output: &self.1 }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OutputRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(values) = value.as_array() {
            if values.len() != 2 {
                return Err(serde::de::Error::custom("legacy output refs require two values"));
            }
            let node_id = values[0].as_str().ok_or_else(|| {
                serde::de::Error::custom("legacy output ref node id must be a string")
            })?;
            let output = values[1].as_str().ok_or_else(|| {
                serde::de::Error::custom("legacy output ref output must be a string")
            })?;
            return Ok(Self(node_id.to_owned(), output.to_owned()));
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            node_id: String,
            output: String,
        }
        serde_json::from_value::<Wire>(value)
            .map(|wire| Self(wire.node_id, wire.output))
            .map_err(serde::de::Error::custom)
    }
}

/// A named input binding with explicit cardinality semantics.
///
/// The tagged representation is the canonical Workflow format. The
/// deserializer also accepts the original `[node_id, output]` form so existing
/// projects can be opened before they are acknowledged by the new authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBinding<T> {
    /// Binds one source to a single-value input.
    Single { source: T },
    /// Binds an ordered collection to a many-value input.
    OrderedMany { sources: Vec<T> },
}

impl<T> InputBinding<T> {
    /// Creates a single-value binding.
    #[must_use]
    pub fn single(source: T) -> Self {
        Self::Single { source }
    }

    /// Creates an ordered-many binding.
    #[must_use]
    pub fn ordered_many(sources: Vec<T>) -> Self {
        Self::OrderedMany { sources }
    }

    /// Visits every source in binding order.
    pub fn sources(&self) -> impl Iterator<Item = &T> {
        match self {
            Self::Single { source } => BindingSources::One(std::iter::once(source)),
            Self::OrderedMany { sources } => BindingSources::Many(sources.iter()),
        }
    }
}

enum BindingSources<'a, T> {
    One(std::iter::Once<&'a T>),
    Many(std::slice::Iter<'a, T>),
}

impl<'a, T> Iterator for BindingSources<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(iterator) => iterator.next(),
            Self::Many(iterator) => iterator.next(),
        }
    }
}

impl<T> Serialize for InputBinding<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire<'a, T> {
            Single { source: &'a T },
            OrderedMany { sources: &'a [T] },
        }

        match self {
            Self::Single { source } => Wire::Single { source },
            Self::OrderedMany { sources } => Wire::OrderedMany { sources },
        }
        .serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for InputBinding<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_array() {
            return serde_json::from_value::<T>(value)
                .map(|source| Self::Single { source })
                .map_err(serde::de::Error::custom);
        }

        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire<T> {
            Single { source: T },
            OrderedMany { sources: Vec<T> },
        }

        serde_json::from_value::<Wire<T>>(value)
            .map(|wire| match wire {
                Wire::Single { source } => Self::Single { source },
                Wire::OrderedMany { sources } => Self::OrderedMany { sources },
            })
            .map_err(serde::de::Error::custom)
    }
}

/// A single node entry in the serialized workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Wiring: input port name -> an explicit single or ordered-many binding.
    #[serde(default)]
    pub inputs: std::collections::BTreeMap<String, InputBinding<OutputRef>>,
    /// UI layout only; ignored by execution.
    #[serde(default)]
    pub position: Option<[f64; 2]>,
}

/// A whole workflow as stored on disk or sent from the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
