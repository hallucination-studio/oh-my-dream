import type {
  AssistantApprovalDecisionInput,
  AssistantPendingWorkflowChange,
  AssistantPresentationEvent,
  AssistantSendInput,
  AssistantSendMessageResult,
  AssistantWorkflowChangeDecisionResult,
} from "./types.ts";

let pendingChange: AssistantPendingWorkflowChange | null = null;
let unavailable = false;
const observers = new Set<(event: AssistantPresentationEvent) => void>();
const INVOCATION_ID = "90000000-0000-4000-8000-000000000001";

export function primeMockAssistantApproval(
  change: AssistantPendingWorkflowChange | null,
): void {
  pendingChange = change;
}

/** Mirrors the backend `provider.unavailable` DesktopErrorDto shape for UI tests. */
export function primeMockAssistantUnavailable(next = true): void {
  unavailable = next;
}

function unavailableError(): Error {
  const error = new Error("The selected generation provider is unavailable.");
  (error as unknown as Record<string, unknown>).code = "provider.unavailable";
  (error as unknown as Record<string, unknown>).retryable = true;
  return error;
}

export async function assistantSendMessage(
  input: AssistantSendInput,
): Promise<AssistantSendMessageResult> {
  if (!input.project_id) throw new Error("Open a project before using the assistant");
  if (unavailable) throw unavailableError();
  emit({
    invocation_id: INVOCATION_ID,
    sequence: "1",
    kind: "text_delta",
    text: "Mock assistant is available only for transport previews.",
  });
  emit({ invocation_id: INVOCATION_ID, sequence: "2", kind: "invocation_completed" });
  return {
    invocation_id: INVOCATION_ID,
    final_text: "Mock assistant is available only for transport previews.",
  };
}

export async function assistantGetPendingWorkflowChange(): Promise<
  AssistantPendingWorkflowChange | null
> {
  return pendingChange;
}

export async function assistantDecideWorkflowChange(
  input: AssistantApprovalDecisionInput,
): Promise<AssistantWorkflowChangeDecisionResult> {
  if (pendingChange === null) throw new Error("assistant.not_found");
  if (
    pendingChange.project_id !== input.project_id
    || pendingChange.workflow_change_id !== input.workflow_change_id
    || pendingChange.approval_scope_id !== input.approval_scope_id
    || pendingChange.mutation_digest_hex !== input.mutation_digest_hex
  ) {
    throw new Error("assistant.approval_mismatch");
  }
  pendingChange = {
    ...pendingChange,
    state: input.decision === "approve" ? "applying" : "rejected",
  };
  return {
    workflow_change_id: pendingChange.workflow_change_id,
    state: pendingChange.state,
  };
}

export async function observeAssistantPresentationEvents(
  onEvent: (event: AssistantPresentationEvent) => void,
): Promise<() => void> {
  observers.add(onEvent);
  return () => observers.delete(onEvent);
}

function emit(event: AssistantPresentationEvent): void {
  for (const observer of observers) observer(event);
}
