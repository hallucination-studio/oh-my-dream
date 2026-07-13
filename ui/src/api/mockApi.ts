// Mock backend — an in-browser stand-in implementing the same WorkflowApi as
// the real Tauri client, so the canvas stays fully interactive with no Rust or
// network (e.g. in a plain `vite dev` browser tab). `selectApi` picks this when
// not running inside a Tauri window.

import type { RunOutput, RunOutputs, RunTerminalStatus, Workflow } from "../workflow/types.ts";
import type { Asset, RunHandle, RunObserver, Skill, WorkflowApi } from "./types.ts";
import { createRunId } from "./runId.ts";

const STEP_MS = 400;

function outputForNodeType(
  type: string,
  nodeId: string,
): { name: string; output: RunOutput } | null {
  switch (type) {
    case "TextToImage":
      return { name: "image", output: { kind: "image", value: `mock://image/${nodeId}` } };
    case "ImageToVideo":
      return { name: "video", output: { kind: "video", value: `mock://video/${nodeId}` } };
    case "TextToAudio":
      return { name: "audio", output: { kind: "audio", value: `mock://audio/${nodeId}` } };
    case "TextPrompt":
      return { name: "text", output: { kind: "string", value: `mock://text/${nodeId}` } };
    default:
      return null;
  }
}

/**
 * Simulates executing `workflow`, emitting running/succeeded transitions.
 * Nodes run in array order (a stand-in for the engine's topological order).
 * Outputs use the same nested nodeId -> outputName shape as the real backend.
 */
function runWorkflow(workflow: Workflow, observe: RunObserver): RunHandle {
  return new MockWorkflowRun(workflow, observe).handle();
}

class MockWorkflowRun {
  private readonly runId = createRunId();
  private readonly timers: ReturnType<typeof setTimeout>[] = [];
  private readonly outputs: RunOutputs = {};
  private started = false;
  private cancelRequested = false;
  private terminal = false;

  constructor(
    private readonly workflow: Workflow,
    private readonly observe: RunObserver,
  ) {
    this.schedule(() => this.start(), 0);
  }

  handle(): RunHandle {
    return { runId: this.runId, cancel: () => this.cancel() };
  }

  private start(): void {
    this.started = true;
    if (this.cancelRequested) {
      this.finish({ state: "cancelled" });
      return;
    }
    if (this.workflow.nodes.length === 0) {
      this.finish({ state: "succeeded", outputs: this.outputs });
      return;
    }
    this.workflow.nodes.forEach((node, index) => {
      this.schedule(() => this.runNode(node.id, node.type, index), STEP_MS * (index + 1));
    });
  }

  private runNode(nodeId: string, nodeType: string, index: number): void {
    if (this.terminal || this.cancelRequested) return;
    this.observe.onProgress({ nodeId, progress: 0.5, nodeState: "running" });
    const produced = outputForNodeType(nodeType, nodeId);
    if (produced) {
      this.outputs[nodeId] = { [produced.name]: produced.output };
    }
    this.observe.onProgress({ nodeId, progress: 1, nodeState: "done" });
    if (index === this.workflow.nodes.length - 1) {
      this.finish({ state: "succeeded", outputs: this.outputs });
    }
  }

  private cancel(): void {
    if (this.terminal || this.cancelRequested) return;
    this.cancelRequested = true;
    this.observe.onStatus({ state: "cancelling" });
    if (!this.started) return;
    this.clearTimers();
    this.schedule(() => this.finish({ state: "cancelled" }), 0);
  }

  private finish(status: RunTerminalStatus): void {
    if (this.terminal) return;
    this.terminal = true;
    this.clearTimers();
    this.observe.onStatus(status);
  }

  private schedule(callback: () => void, delay: number): void {
    this.timers.push(setTimeout(callback, delay));
  }

  private clearTimers(): void {
    this.timers.forEach(clearTimeout);
  }
}

// The mock has no persistent store; asset listing is empty until a real backend
// is present. This keeps the interface total rather than throwing.
async function listAssets(): Promise<Asset[]> {
  return [];
}

async function assetsRoot(): Promise<string | null> {
  return null;
}

async function getAsset(id: string): Promise<Asset> {
  throw new Error(`Mock backend has no asset store; cannot fetch asset ${id}`);
}

async function listProjects() {
  return [{ id: "default", name: "Default", created_at: 0 }];
}

async function createProject(name: string) {
  return { id: "default", name, created_at: 0 };
}

async function openProject(id: string) {
  return {
    project: { id, name: "Default", created_at: 0 },
    workflow_json: { version: "1.0", project_id: id, nodes: [] },
  };
}

async function saveWorkflow(): Promise<void> {}

async function loadWorkflow(projectId: string): Promise<Workflow> {
  return { version: "1.0", project_id: projectId, nodes: [] };
}

async function getProviders() {
  return [{ id: "mock", name: "Mock", active: true, has_key: false }];
}

async function setActiveProvider(): Promise<void> {}

async function setProviderKey(): Promise<void> {}

async function getAssistantConfig() {
  return {
    enabled: false,
    base_url: "https://api.openai.com/v1",
    model: "gpt-5.4",
    has_key: false,
    temperature: 0.3,
    max_tool_iters: 20,
    system_prompt_extra: null,
    developer_mode: false,
    skills: { installed: [], enabled: [] },
  };
}

async function setAssistantConfig(): Promise<void> {}

async function getAssistantSession() {
  return { port: 0, token: "" };
}

async function getCapabilityManifest() {
  return { capabilities: [] };
}

async function listSkills() {
  return [];
}

async function installSkill(): Promise<Skill> {
  throw new Error("Mock backend cannot install assistant skills");
}

async function setSkillEnabled(): Promise<void> {}

async function uninstallSkill(): Promise<void> {}

export const mockApi: WorkflowApi = {
  runWorkflow,
  assetsRoot,
  listAssets,
  getAsset,
  listProjects,
  createProject,
  openProject,
  saveWorkflow,
  loadWorkflow,
  getProviders,
  setActiveProvider,
  setProviderKey,
  getAssistantConfig,
  setAssistantConfig,
  getAssistantSession,
  getCapabilityManifest,
  listSkills,
  installSkill,
  setSkillEnabled,
  uninstallSkill,
};
