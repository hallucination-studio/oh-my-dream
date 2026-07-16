//! Concrete Desktop time and identity adapters for Workflow consumer interfaces.

use std::time::{SystemTime, UNIX_EPOCH};

use engine::node_capability::{WorkflowNodeExecutionId, WorkflowRunId};
use engine::workflow::{
    WorkflowApplicationError, WorkflowClockInterface, WorkflowIdentityGeneratorInterface,
    WorkflowRunTime,
};
use engine::workflow_graph::WorkflowId;

/// UTC system-clock implementation of the Workflow time boundary.
#[derive(Clone, Copy)]
pub struct SystemWorkflowClockAdapterImpl;

impl WorkflowClockInterface for SystemWorkflowClockAdapterImpl {
    fn current_workflow_time(&self) -> Result<WorkflowRunTime, WorkflowApplicationError> {
        let milliseconds =
            SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| persistence())?.as_millis();
        let milliseconds = i64::try_from(milliseconds).map_err(|_| persistence())?;
        WorkflowRunTime::from_utc_milliseconds(milliseconds).map_err(Into::into)
    }
}

/// Operating-system-random UUIDv4 implementation of all Workflow identity methods.
#[derive(Clone, Copy)]
pub struct UuidV4WorkflowIdentityGeneratorAdapterImpl;

impl WorkflowIdentityGeneratorInterface for UuidV4WorkflowIdentityGeneratorAdapterImpl {
    fn generate_workflow_id(&self) -> WorkflowId {
        loop {
            if let Ok(value) = WorkflowId::from_uuid(uuid::Uuid::new_v4()) {
                return value;
            }
        }
    }

    fn generate_workflow_run_id(&self) -> WorkflowRunId {
        loop {
            if let Some(value) = WorkflowRunId::from_uuid(uuid::Uuid::new_v4()) {
                return value;
            }
        }
    }

    fn generate_workflow_node_execution_id(&self) -> WorkflowNodeExecutionId {
        loop {
            if let Some(value) = WorkflowNodeExecutionId::from_uuid(uuid::Uuid::new_v4()) {
                return value;
            }
        }
    }
}

fn persistence() -> WorkflowApplicationError {
    WorkflowApplicationError::WorkflowPersistenceFailure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_returns_a_non_negative_utc_millisecond_value() {
        let observed = SystemWorkflowClockAdapterImpl.current_workflow_time().unwrap();
        assert!(observed.as_utc_milliseconds() > 0);
    }

    #[test]
    fn generator_returns_distinct_rfc_uuid_version_four_values_for_every_method() {
        let generator = UuidV4WorkflowIdentityGeneratorAdapterImpl;
        let workflow = generator.generate_workflow_id().as_uuid();
        let run = generator.generate_workflow_run_id().as_uuid();
        let execution = generator.generate_workflow_node_execution_id().as_uuid();

        for value in [workflow, run, execution] {
            assert_eq!(value.get_version(), Some(uuid::Version::Random));
            assert_eq!(value.get_variant(), uuid::Variant::RFC4122);
        }
        assert_ne!(workflow, run);
        assert_ne!(run, execution);
        assert_ne!(workflow, execution);
    }
}
