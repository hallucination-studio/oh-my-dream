import { describe, expect, it, vi } from "vitest";
import { createCapabilityExecutor } from "./capabilityExecutor.ts";
import type { WorkflowApi } from "../api/index.ts";

describe("createCapabilityExecutor", () => {
  it("dispatches UI capabilities through the supplied canvas actions", async () => {
    const actions = canvasActions();
    const executor = createCapabilityExecutor({ actions, api: workflowApi() });

    await executor.execute("workflow.add_node", {
      node_type: "TextPrompt",
      position: { x: 10, y: 20 },
    });
    await executor.execute("workflow.set_param", {
      node_id: "n1",
      name: "text",
      value: "a red fox",
    });
    await executor.execute("workflow.switch_tab", { tab: "assets" });

    expect(actions.addNode).toHaveBeenCalledWith("TextPrompt", { x: 10, y: 20 });
    expect(actions.setParam).toHaveBeenCalledWith("n1", "text", "a red fox");
    expect(actions.switchTab).toHaveBeenCalledWith("assets");
  });

  it("dispatches backend capabilities through WorkflowApi", async () => {
    const api = workflowApi();
    const executor = createCapabilityExecutor({ actions: canvasActions(), api });

    await executor.execute("project.create", { name: "Launch" });
    await executor.execute("provider.set_key", { provider_id: "fal", key: "secret" });
    await executor.execute("asset.list", { kind: "image", sort: "newest" });

    expect(api.createProject).toHaveBeenCalledWith("Launch");
    expect(api.setProviderKey).toHaveBeenCalledWith("fal", "secret");
    expect(api.listAssets).toHaveBeenCalledWith({ kind: "image", sort: "newest" });
  });

  it("returns errors for unknown capabilities", async () => {
    const executor = createCapabilityExecutor({ actions: canvasActions(), api: workflowApi() });

    await expect(executor.execute("missing.capability", {})).rejects.toThrow("Unknown capability");
  });
});

function canvasActions() {
  return {
    addNode: vi.fn(),
    connectNodes: vi.fn(),
    setParam: vi.fn(),
    deleteNode: vi.fn(),
    selectNode: vi.fn(),
    switchTab: vi.fn(),
    getCanvasState: vi.fn(() => ({ nodes: [], edges: [] })),
    getSelection: vi.fn(() => null),
  };
}

function workflowApi(): WorkflowApi {
  return {
    runWorkflow: vi.fn(),
    assetsRoot: vi.fn(),
    listAssets: vi.fn(),
    getAsset: vi.fn(),
    listProjects: vi.fn(),
    createProject: vi.fn(),
    openProject: vi.fn(),
    saveWorkflow: vi.fn(),
    loadWorkflow: vi.fn(),
    getProviders: vi.fn(),
    setActiveProvider: vi.fn(),
    setProviderKey: vi.fn(),
    getAssistantConfig: vi.fn(),
    setAssistantConfig: vi.fn(),
    getAssistantSession: vi.fn(),
    getCapabilityManifest: vi.fn(),
    listSkills: vi.fn(),
    installSkill: vi.fn(),
    setSkillEnabled: vi.fn(),
    uninstallSkill: vi.fn(),
  };
}
