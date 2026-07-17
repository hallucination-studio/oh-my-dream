import { beforeEach, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
});

it("uses the three canonical Assistant commands", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  invokeMock.mockResolvedValue({});

  await tauriApi.assistantSendMessage({
    project_id: "10000000-0000-4000-8000-000000000001",
    workflow_present: false,
    workflow_revision: null,
    selected_node_ids: [],
    selected_asset_ids: [],
    text: "Build a film",
  });
  await tauriApi.assistantGetPendingWorkflowChange("10000000-0000-4000-8000-000000000001");
  await tauriApi.assistantDecideWorkflowChange({
    project_id: "10000000-0000-4000-8000-000000000001",
    workflow_change_id: "20000000-0000-4000-8000-000000000001",
    approval_scope_id: "30000000-0000-4000-8000-000000000001",
    mutation_digest_hex: "00".repeat(32),
    decision: "approve",
  });

  expect(invokeMock.mock.calls.map(([command]) => command)).toEqual([
    "assistant_send_message",
    "assistant_get_pending_workflow_change",
    "assistant_decide_workflow_change",
  ]);
});

it("subscribes only to the typed presentation event", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const unlisten = vi.fn();
  listenMock.mockResolvedValue(unlisten);
  const observer = vi.fn();

  await expect(tauriApi.observeAssistantPresentationEvents(observer)).resolves.toBe(unlisten);
  expect(listenMock).toHaveBeenCalledWith(
    "assistant-presentation-event-v1",
    expect.any(Function),
  );
});
