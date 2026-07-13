// Real backend client: invokes the src-tauri commands over Tauri IPC.
//
// Workflow execution uses one ordered Channel per invocation. The command
// result is the sole terminal authority; cancellation remains a request until
// that result reports cancelled, succeeded, or failed.

import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { Workflow } from "../workflow/types.ts";
import { createRunId } from "./runId.ts";
import type {
  AssetDto,
  AssistantConfig,
  AssistantConfigInput,
  AssistantSession,
  CancelWorkflowRunResult,
  CapabilityManifest,
  ListAssetsOptions,
  Project,
  ProjectWorkspace,
  Provider,
  RunHandle,
  RunObserver,
  Skill,
  WorkflowApi,
  WorkflowRunEvent,
  WorkflowRunResult,
} from "./types.ts";

function runWorkflow(workflow: Workflow, observe: RunObserver): RunHandle {
  return new TauriWorkflowRun(workflow, observe).handle();
}

class TauriWorkflowRun {
  private readonly runId = createRunId();
  private readonly channel = new Channel<WorkflowRunEvent>();
  private started = false;
  private cancelRequested = false;
  private cancelSent = false;
  private terminal = false;
  private cancelFailure: string | null = null;

  constructor(
    private readonly workflow: Workflow,
    private readonly observe: RunObserver,
  ) {
    this.channel.onmessage = (event) => this.handleEvent(event);
    this.start();
  }

  handle(): RunHandle {
    return { runId: this.runId, cancel: () => this.cancel() };
  }

  private start(): void {
    void invoke<WorkflowRunResult>("start_workflow_run", {
      run_id: this.runId,
      workflow_json: JSON.stringify(this.workflow),
      on_event: this.channel,
    })
      .then((result) => this.settle(result))
      .catch((error: unknown) => this.fail(this.failureReason(error)));
  }

  private handleEvent(event: WorkflowRunEvent): void {
    if (this.terminal || event.run_id !== this.runId) return;
    if (event.event === "started") {
      this.started = true;
      this.requestCancellation();
      return;
    }
    const committed = event.node.state === "done" || event.node.state === "cached";
    if (this.cancelRequested && !committed) return;
    this.observe.onProgress({
      nodeId: event.node.node_id,
      progress: event.node.progress ?? (committed ? 1 : 0),
      nodeState: event.node.state,
      cost: event.node.cost ?? undefined,
    });
  }

  private cancel(): void {
    if (this.terminal || this.cancelRequested) return;
    this.cancelRequested = true;
    this.observe.onStatus({ state: "cancelling" });
    this.requestCancellation();
  }

  private requestCancellation(): void {
    if (!this.started || !this.cancelRequested || this.cancelSent || this.terminal) return;
    this.cancelSent = true;
    void invoke<CancelWorkflowRunResult>("cancel_workflow_run", { run_id: this.runId })
      .then((result) => {
        if (result.run_id !== this.runId) {
          this.handleCancellationFailure("cancel_workflow_run returned a different run_id");
        } else {
          this.cancelFailure = null;
        }
      })
      .catch((error: unknown) => {
        this.handleCancellationFailure(String(error));
      });
  }

  private handleCancellationFailure(reason: string): void {
    if (this.terminal || !this.cancelRequested) return;
    this.cancelFailure = reason;
    this.cancelRequested = false;
    this.cancelSent = false;
    this.observe.onStatus({ state: "cancel_failed", reason });
  }

  private settle(result: WorkflowRunResult): void {
    if (this.terminal) return;
    if (result.run_id !== this.runId) {
      this.fail(`Workflow run identity mismatch: expected ${this.runId}, received ${result.run_id}`);
      return;
    }
    this.terminal = true;
    if (result.status === "succeeded") {
      this.observe.onStatus({ state: "succeeded", outputs: result.outputs });
    } else if (result.status === "cancelled") {
      this.observe.onStatus({ state: "cancelled" });
    } else {
      this.observe.onStatus({ state: "failed", reason: result.reason });
    }
  }

  private fail(reason: string): void {
    if (this.terminal) return;
    this.terminal = true;
    this.observe.onStatus({ state: "failed", reason });
  }

  private failureReason(error: unknown): string {
    return this.cancelFailure === null
      ? String(error)
      : `Run failed after cancellation request (${this.cancelFailure}): ${String(error)}`;
  }
}

async function listAssets(options: ListAssetsOptions = {}): Promise<AssetDto[]> {
  const root = await assetsRoot();
  const assets = await invoke<AssetDto[]>("list_assets", {
    kind: options.kind ?? null,
    project_id: options.project_id ?? null,
    model: options.model ?? null,
    prompt: options.prompt ?? null,
    sort: options.sort ?? null,
  });
  return assets.map((asset) => convertAssetPaths(asset, root));
}

async function getAsset(id: string): Promise<AssetDto> {
  const root = await assetsRoot();
  const asset = await invoke<AssetDto>("get_asset", { id });
  return convertAssetPaths(asset, root);
}

async function assetsRoot(): Promise<string> {
  return invoke<string>("assets_root");
}

async function listProjects(): Promise<Project[]> {
  return invoke<Project[]>("list_projects");
}

async function createProject(name: string): Promise<Project> {
  return invoke<Project>("create_project", { name });
}

async function openProject(id: string): Promise<ProjectWorkspace> {
  return invoke<ProjectWorkspace>("open_project", { id });
}

async function saveWorkflow(workflow: Workflow): Promise<void> {
  await invoke("save_workflow", { workflow_json: JSON.stringify(workflow) });
}

async function loadWorkflow(projectId: string): Promise<Workflow> {
  return invoke<Workflow>("load_workflow", { project_id: projectId });
}

async function getProviders(): Promise<Provider[]> {
  return invoke<Provider[]>("get_providers");
}

async function setActiveProvider(providerId: string): Promise<void> {
  await invoke("set_active_provider", { provider_id: providerId });
}

async function setProviderKey(providerId: string, key: string): Promise<void> {
  await invoke("set_provider_key", { provider_id: providerId, key });
}

async function getAssistantConfig(): Promise<AssistantConfig> {
  return invoke<AssistantConfig>("get_assistant_config");
}

async function setAssistantConfig(input: AssistantConfigInput): Promise<void> {
  await invoke("set_assistant_config", { input });
}

async function getAssistantSession(): Promise<AssistantSession> {
  return invoke<AssistantSession>("get_assistant_session");
}

async function getCapabilityManifest(): Promise<CapabilityManifest> {
  return invoke<CapabilityManifest>("get_capability_manifest");
}

async function listSkills(): Promise<Skill[]> {
  return invoke<Skill[]>("list_skills");
}

async function installSkill(path: string): Promise<Skill> {
  return invoke<Skill>("install_skill", { path });
}

async function setSkillEnabled(name: string, enabled: boolean): Promise<void> {
  await invoke("set_skill_enabled", { name, enabled });
}

async function uninstallSkill(name: string): Promise<void> {
  await invoke("uninstall_skill", { name });
}

function convertAssetPaths(asset: AssetDto, root: string | null): AssetDto {
  return {
    ...asset,
    file_path: convertRootedPath(asset.file_path, root),
    thumbnail_path:
      asset.thumbnail_path === null ? null : convertRootedPath(asset.thumbnail_path, root),
  };
}

function convertRootedPath(path: string, root: string | null): string {
  if (!root || !isPathUnderRoot(path, root)) {
    return path;
  }
  return convertFileSrc(path);
}

function isPathUnderRoot(path: string, root: string): boolean {
  const normalizedRoot = root.replace(/[\\/]+$/, "");
  return (
    path === normalizedRoot ||
    path.startsWith(`${normalizedRoot}/`) ||
    path.startsWith(`${normalizedRoot}\\`)
  );
}

export const tauriApi: WorkflowApi = {
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
