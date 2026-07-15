import type {
  AssistantApprovalDecisionInput,
  AssistantPendingApproval,
  AssistantSendInput,
  ResponsesStreamEvent,
  WorkflowHead,
} from "./types.ts";

let pendingApproval: AssistantPendingApproval | null = null;

export function primeMockAssistantApproval(approval: AssistantPendingApproval | null): void {
  pendingApproval = approval;
}

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
  return pendingApproval;
}

export async function decideAssistantApproval(
  _input: AssistantApprovalDecisionInput,
  onEvent: (event: ResponsesStreamEvent) => void,
): Promise<WorkflowHead | null> {
  if (pendingApproval === null) throw new Error("ASSISTANT_APPROVAL_NOT_FOUND");
  if (pendingApproval.project_id !== _input.project_id) {
    throw new Error("ASSISTANT_APPROVAL_SCOPE_MISMATCH");
  }
  if (pendingApproval.approval_scope_id !== _input.approval_scope_id) {
    throw new Error("ASSISTANT_APPROVAL_STALE");
  }
  if (pendingApproval.candidate_digest !== _input.candidate_digest) {
    throw new Error("ASSISTANT_APPROVAL_STALE");
  }
  pendingApproval = null;
  onEvent({ type: "response.completed" });
  return null;
}
