use std::time::Duration;

use assistant::{
    application::AssistantToolExecutionContext,
    domain::AssistantModelInvocationId,
    interfaces::{
        AssistantApplicationError, AssistantModelResumeRequest, AssistantModelRunnerInterface,
        AssistantModelTurnRequest, AssistantModelTurnResult,
    },
    protocol_v1::AssistantProtocolFrame,
};
use async_trait::async_trait;
use tokio::time::timeout;

use super::{
    AssistantPresentationEventPayload, AssistantPresentationEventPublisherInterface,
    AssistantProtocolProcessLauncherInterface, AssistantProtocolToolExecutorInterface,
    AssistantReviewerProtocolInterface, AssistantToolExecutionContextFactoryInterface,
    frames::{resume_frame, start_frame},
    runner::PythonAgentsAssistantModelRunnerAdapterImpl,
};

const INVOCATION_DEADLINE: Duration = Duration::from_secs(10 * 60);

#[async_trait]
impl<L, T, F, P, Q> AssistantModelRunnerInterface
    for PythonAgentsAssistantModelRunnerAdapterImpl<L, T, F, P, Q>
where
    L: AssistantProtocolProcessLauncherInterface,
    T: AssistantProtocolToolExecutorInterface,
    F: AssistantToolExecutionContextFactoryInterface,
    P: AssistantPresentationEventPublisherInterface,
    Q: AssistantReviewerProtocolInterface,
{
    async fn start_assistant_model_turn(
        &self,
        request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let context = self.context_factory.create_assistant_tool_execution_context(&request)?;
        self.run(request.invocation_id, start_frame(&request)?, context).await
    }

    async fn resume_assistant_model_turn(
        &self,
        request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let context =
            self.context_factory.create_resumed_assistant_tool_execution_context(&request)?;
        self.run(request.invocation_id, resume_frame(&request)?, context).await
    }
}

impl<L, T, F, P, Q> PythonAgentsAssistantModelRunnerAdapterImpl<L, T, F, P, Q>
where
    L: AssistantProtocolProcessLauncherInterface,
    T: AssistantProtocolToolExecutorInterface,
    F: AssistantToolExecutionContextFactoryInterface,
    P: AssistantPresentationEventPublisherInterface,
    Q: AssistantReviewerProtocolInterface,
{
    async fn run(
        &self,
        invocation_id: AssistantModelInvocationId,
        first_frame: AssistantProtocolFrame,
        context: AssistantToolExecutionContext,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let invocation_text = first_frame.invocation_id.clone();
        let mut sequence = 0;
        let mut process = self.launcher.launch_assistant_protocol_process().await?;
        let result = timeout(
            INVOCATION_DEADLINE,
            self.exchange(&mut *process, invocation_id, first_frame, context, &mut sequence),
        )
        .await;
        let outcome = self.finish_process(&mut *process, result).await;
        if let Err(error) = &outcome {
            tracing::warn!(invocation_id = invocation_text, "Assistant model invocation failed");
            self.publish(
                invocation_id,
                &mut sequence,
                AssistantPresentationEventPayload::InvocationFailed { error: *error },
            )
            .await?;
        }
        outcome
    }

    async fn finish_process(
        &self,
        process: &mut dyn super::AssistantProtocolProcessInterface,
        result: Result<
            Result<AssistantModelTurnResult, AssistantApplicationError>,
            tokio::time::error::Elapsed,
        >,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        match result {
            Ok(Ok(value)) => {
                match timeout(INVOCATION_DEADLINE, process.shutdown_assistant_protocol_process())
                    .await
                {
                    Ok(Ok(())) => Ok(value),
                    Ok(Err(error)) => {
                        process.abort_assistant_protocol_process().await;
                        Err(error)
                    }
                    Err(_) => {
                        process.abort_assistant_protocol_process().await;
                        Err(AssistantApplicationError::DeadlineExceeded)
                    }
                }
            }
            Ok(Err(error)) => {
                process.abort_assistant_protocol_process().await;
                Err(error)
            }
            Err(_) => {
                process.abort_assistant_protocol_process().await;
                Err(AssistantApplicationError::DeadlineExceeded)
            }
        }
    }
}
