import { beforeEach, expect, it, vi } from "vitest";
import {
  assistantDecideWorkflowChange,
  assistantGetPendingWorkflowChange,
  assistantSendMessage,
  observeAssistantPresentationEvents,
  primeMockAssistantApproval,
} from "./mockAssistant.ts";
import type { AssistantPendingWorkflowChange } from "./types.ts";

const CHANGE: AssistantPendingWorkflowChange = {
  workflow_change_id: "20000000-0000-4000-8000-000000000001",
  project_id: "10000000-0000-4000-8000-000000000001",
  base_workflow_revision: "1",
  mutation_digest_hex: "00".repeat(32),
  approval_scope_id: "30000000-0000-4000-8000-000000000001",
  expires_at_epoch_ms: "1000",
  state: "awaiting_approval",
  lineage: {
    kind: "user_message",
    invocation_id: "40000000-0000-4000-8000-000000000001",
    intent: "Build a film",
  },
  mutations: [{}],
  readiness_issues: [],
};

beforeEach(() => primeMockAssistantApproval(null));

it("emits typed contiguous presentation events", async () => {
  const observer = vi.fn();
  const unlisten = await observeAssistantPresentationEvents(observer);
  await assistantSendMessage({
    project_id: CHANGE.project_id,
    workflow_present: false,
    workflow_revision: null,
    selected_node_ids: [],
    selected_asset_ids: [],
    text: "Build a film",
  });
  unlisten();
  expect(observer.mock.calls.map(([event]) => event.kind)).toEqual([
    "text_delta",
    "invocation_completed",
  ]);
});

it.each(["approve", "reject"] as const)("commits an exact %s decision", async (decision) => {
  primeMockAssistantApproval(CHANGE);
  const result = await assistantDecideWorkflowChange({
    project_id: CHANGE.project_id,
    workflow_change_id: CHANGE.workflow_change_id,
    approval_scope_id: CHANGE.approval_scope_id,
    mutation_digest_hex: CHANGE.mutation_digest_hex,
    decision,
  });
  expect(result.state).toBe(decision === "approve" ? "applying" : "rejected");
  await expect(assistantGetPendingWorkflowChange()).resolves.toMatchObject({
    workflow_change_id: result.workflow_change_id,
    state: result.state,
  });
});
