//! Node registry: maps a node `type_id` to a factory that builds an instance
//! from its serialized parameters.

use crate::error::EngineError;
use crate::node::Node;
use std::collections::HashMap;

/// Parameters for a node as stored in the workflow JSON (`params` object).
pub type NodeParams = serde_json::Map<String, serde_json::Value>;

/// A factory that constructs a [`Node`] from its serialized parameters.
///
/// Returns a boxed error on invalid params; the caller attaches node context.
pub type NodeFactory =
    Box<dyn Fn(&NodeParams) -> Result<Box<dyn Node>, crate::node::NodeRunError> + Send + Sync>;

/// Registry of known node types, populated at startup by the `nodes` crate.
#[derive(Default)]
pub struct NodeRegistry {
    factories: HashMap<String, NodeFactory>,
}

impl NodeRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a factory under `type_id`, replacing any previous entry.
    pub fn register(&mut self, type_id: impl Into<String>, factory: NodeFactory) {
        self.factories.insert(type_id.into(), factory);
    }

    /// Instantiates a node of `type_id` from `params`.
    ///
    /// `node_id` is used only to enrich errors with graph context.
    pub fn instantiate(
        &self,
        node_id: &str,
        type_id: &str,
        params: &NodeParams,
    ) -> Result<Box<dyn Node>, EngineError> {
        let factory =
            self.factories
                .get(type_id)
                .ok_or_else(|| EngineError::UnknownNodeType {
                    node_id: node_id.to_owned(),
                    type_id: type_id.to_owned(),
                })?;
        factory(params).map_err(|source| EngineError::NodeExecution {
            node_id: node_id.to_owned(),
            type_id: type_id.to_owned(),
            source,
        })
    }

    /// Returns whether a `type_id` is registered.
    #[must_use]
    pub fn contains(&self, type_id: &str) -> bool {
        self.factories.contains_key(type_id)
    }
}
