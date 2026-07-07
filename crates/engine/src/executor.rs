//! Executor and result-cache contracts.
//!
//! Wave 0 defines the surface only; the topological scheduling, type-checked
//! wiring, and cache logic are implemented in Wave 1. Bodies are left as
//! `todo!()` so the crate compiles and downstream crates can build against a
//! stable API.

use crate::error::Result;
use crate::graph::Workflow;
use crate::registry::NodeRegistry;
use crate::value::ValueMap;
use std::collections::BTreeMap;

/// Outputs produced by a single run, keyed by node id.
pub type RunOutputs = BTreeMap<String, ValueMap>;

/// Reuses node outputs across runs when a node's resolved inputs and params are
/// unchanged, avoiding re-execution of expensive downstream work.
///
/// The concrete hashing/storage strategy is implemented in Wave 1.
#[derive(Default)]
pub struct ResultCache {
    _private: (),
}

impl ResultCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Builds an execution plan from a [`Workflow`], validating wiring and ordering
/// nodes topologically, then runs nodes in order consulting the [`ResultCache`].
pub struct Executor<'r> {
    _registry: &'r NodeRegistry,
}

impl<'r> Executor<'r> {
    /// Creates an executor bound to a node registry.
    #[must_use]
    pub fn new(registry: &'r NodeRegistry) -> Self {
        Self { _registry: registry }
    }

    /// Validates and executes `workflow`, returning each node's outputs.
    ///
    /// Wave 1 fills this in: instantiate nodes, type-check wiring, detect
    /// cycles, order topologically, resolve inputs, consult `cache`, run.
    pub fn execute(&self, _workflow: &Workflow, _cache: &mut ResultCache) -> Result<RunOutputs> {
        todo!("Wave 1: topological execution with type-checked wiring and caching")
    }
}
