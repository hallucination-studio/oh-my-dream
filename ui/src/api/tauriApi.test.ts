import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: (path: string) => path,
  Channel: class<T> {
    onmessage = (_event: T) => undefined;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
});

describe("canonical Workflow Tauri client", () => {
  it("invokes canonical get and mutation requests", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock.mockResolvedValue({});
    await tauriApi.workflowGetCurrent("project");
    await tauriApi.workflowApplyMutation("project", "workflow", "7", [{
      kind: "remove_node",
      node_id: "node",
    }]);

    expect(invokeMock.mock.calls[0]).toEqual([
      "workflow_get_current",
      { request: { project_id: "project" } },
    ]);
    expect(invokeMock.mock.calls[1]?.[0]).toBe("workflow_apply_mutation");
    expect(invokeMock.mock.calls[1]?.[1]).toMatchObject({
      request: {
        project_id: "project",
        workflow_id: "workflow",
        base_revision: "7",
        actions: [{ kind: "remove_node", node_id: "node" }],
      },
    });
  });

  it("uses the closed WholeWorkflow scope for Run admission", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock.mockResolvedValue({});
    await tauriApi.workflowStartRun("project", "workflow", "8", {
      kind: "whole_workflow",
    });
    expect(invokeMock).toHaveBeenCalledWith("workflow_start_run", {
      request: expect.objectContaining({
        project_id: "project",
        workflow_id: "workflow",
        workflow_revision: "8",
        scope: { kind: "whole_workflow" },
      }),
    });
  });

  it("subscribes to the exact durable event name", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    listenMock.mockResolvedValue(() => undefined);
    await tauriApi.observeWorkflowRunEvents(vi.fn());
    expect(listenMock).toHaveBeenCalledWith("workflow-run-event-v1", expect.any(Function));
  });
});
