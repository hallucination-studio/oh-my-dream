//! Concrete Desktop adapters for Assistant-owned interfaces.

mod clock;
mod continuation;
mod production_plan;
mod workflow_change;

pub use clock::SystemAssistantClockAdapterImpl;
pub use continuation::LocalFilesystemAssistantModelContinuationStoreAdapterImpl;
pub use production_plan::SqliteAssistantProductionPlanRepositoryAdapterImpl;
pub use workflow_change::SqliteAssistantWorkflowChangeRepositoryAdapterImpl;

#[cfg(test)]
mod tests;
