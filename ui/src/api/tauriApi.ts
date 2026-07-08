// Real backend client: invokes the src-tauri commands over Tauri IPC.
//
// run_workflow is synchronous on the backend (runs to completion and returns
// final outputs), while the backend emits node_progress events during the run.
// Cancellation is best-effort: the backend has no cancel command yet, so cancel
// only stops us from reporting later updates to the UI.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { NodeProgressEvent, RunOutputs, Workflow } from "../workflow/types.ts";
import type {
  Asset,
  ListAssetsOptions,
  Project,
  ProjectWorkspace,
  Provider,
  RunHandle,
  RunObserver,
  WorkflowApi,
} from "./types.ts";

interface RunWorkflowResultDto {
  outputs: RunOutputs;
}

function runWorkflow(workflow: Workflow, observe: RunObserver): RunHandle {
  let cancelled = false;
  let finished = false;
  let unlisten: UnlistenFn | null = null;

  listen<NodeProgressEvent>("node_progress", (event) => {
    if (cancelled) {
      return;
    }
    const progress = event.payload.progress ?? (event.payload.state === "done" ? 1 : 0);
    observe({
      state: "running",
      nodeId: event.payload.node_id,
      progress,
      nodeState: event.payload.state,
      cost: event.payload.cost ?? undefined,
    });
  })
    .then((dispose) => {
      if (cancelled || finished) {
        dispose();
        return null;
      }
      unlisten = dispose;
      return invoke<RunWorkflowResultDto>("run_workflow", {
        workflow_json: JSON.stringify(workflow),
      });
    })
    .then((result) => {
      if (!result || cancelled) {
        return;
      }
      finished = true;
      unlisten?.();
      observe({ state: "succeeded", outputs: result.outputs });
    })
    .catch((error: unknown) => {
      if (cancelled) {
        return;
      }
      finished = true;
      unlisten?.();
      // Surface the backend error string verbatim rather than swallowing it.
      observe({ state: "failed", reason: String(error) });
    });

  return {
    cancel: () => {
      cancelled = true;
      finished = true;
      unlisten?.();
      observe({ state: "failed", reason: "Run cancelled by user" });
    },
  };
}

async function listAssets(options: ListAssetsOptions = {}): Promise<Asset[]> {
  const root = await assetsRoot();
  const assets = await invoke<Asset[]>("list_assets", {
    kind: options.kind ?? null,
    project_id: options.project_id ?? null,
    model: options.model ?? null,
    prompt: options.prompt ?? null,
    sort: options.sort ?? null,
  });
  return assets.map((asset) => convertAssetPaths(asset, root));
}

async function getAsset(id: string): Promise<Asset> {
  const root = await assetsRoot();
  const asset = await invoke<Asset>("get_asset", { id });
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

function convertAssetPaths(asset: Asset, root: string | null): Asset {
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
};
