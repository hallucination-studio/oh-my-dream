import type {
  AssistantApprovalDecisionInput,
  AssistantPendingApproval,
  AssistantSendInput,
  ResponsesStreamEvent,
  WorkflowHead,
} from "./types.ts";

export async function sendAssistant(
  input: AssistantSendInput,
  onEvent: (event: ResponsesStreamEvent) => void,
): Promise<WorkflowHead | null> {
  if (!input.project_id) throw new Error("Open a project before using the assistant");
  onEvent({
    type: "response.output_text.delta",
    delta: "Mock assistant is available only for transport previews.",
  });
  onEvent({ type: "response.completed" });
  return null;
}

export async function getPendingAssistantApproval(): Promise<AssistantPendingApproval | null> {
  return null;
}

export async function decideAssistantApproval(
  _input: AssistantApprovalDecisionInput,
  onEvent: (event: ResponsesStreamEvent) => void,
): Promise<WorkflowHead | null> {
  onEvent({ type: "response.completed" });
  return null;
}
