//! Assistant-to-Workflow canonical Desktop bridges.

mod proposal;
mod receipt;
mod run_projection;
#[cfg(test)]
mod tests;
mod workflow;
mod workspace;

pub use proposal::translate_proposals;
pub use workflow::DesktopAssistantWorkflowBridgeAdapterImpl;
pub use workspace::DesktopAssistantWorkspaceBridgeAdapterImpl;
