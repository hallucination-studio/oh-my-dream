//! Consumer-owned Assistant substitution boundaries.

mod bridge;
mod model;
mod proposal;
mod repository;
mod value;

pub use crate::domain::{
    AssistantWorkflowApplyReceiptBoundaryValue, AssistantWorkflowRunBoundaryValue,
};
pub use bridge::*;
pub use model::*;
pub use proposal::*;
pub use repository::*;
pub use value::*;

#[cfg(test)]
mod fault_tests;
#[cfg(test)]
mod tests;
