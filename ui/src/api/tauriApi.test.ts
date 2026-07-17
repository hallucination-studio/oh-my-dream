import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
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

  it("invokes the two closed Generation Provider Settings commands", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock.mockResolvedValue({});
    await tauriApi.generationProviderSettingsGet();
    await tauriApi.generationProviderSettingsApply("4", {
      kind: "remove_binding",
      profile_ref: "image.high_quality_general@1",
      generation_kind: "image",
    });

    expect(invokeMock.mock.calls.slice(-2)).toEqual([
      ["generation_provider_settings_get", { request: {} }],
      ["generation_provider_settings_apply", {
        request: {
          expected_settings_revision: "4",
          action: {
            kind: "remove_binding",
            profile_ref: "image.high_quality_general@1",
            generation_kind: "image",
          },
        },
      }],
    ]);
  });

  it("invokes the four canonical Asset commands without paths or bytes", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock.mockResolvedValue({});

    await tauriApi.assetImport("project", "image");
    await tauriApi.assetGet("project", "asset");
    await tauriApi.assetList("project", "video", "cursor", 25);
    await tauriApi.assetIssuePreview("project", "asset");

    expect(invokeMock.mock.calls).toEqual([
      ["asset_import", {
        request: { project_id: "project", expected_media_kind: "image" },
      }],
      ["asset_get", {
        request: { project_id: "project", asset_id: "asset" },
      }],
      ["asset_list", {
        request: {
          project_id: "project",
          media_kind: "video",
          cursor: "cursor",
          limit: 25,
        },
      }],
      ["asset_issue_preview", {
        request: { project_id: "project", asset_id: "asset" },
      }],
    ]);
  });
});
