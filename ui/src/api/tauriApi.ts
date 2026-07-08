// Real backend client: invokes the src-tauri commands over Tauri IPC.
//
// run_workflow is synchronous on the backend (runs to completion and returns
// final outputs), so here we emit a single "running" transition, await the
// command, then a terminal "succeeded"/"failed". Cancellation is best-effort:
// the backend has no cancel command yet, so cancel only stops us from
// reporting the result to the UI.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { RunOutputs, Workflow } from "../workflow/types.ts";
import type { Asset, RunHandle, RunObserver, WorkflowApi } from "./types.ts";

interface RunWorkflowResultDto {
  outputs: RunOutputs;
}

function runWorkflow(workflow: Workflow, observe: RunObserver): RunHandle {
  let cancelled = false;

  const firstNode = workflow.nodes[0]?.id ?? "";
  observe({ state: "running", nodeId: firstNode, progress: 0 });

  invoke<RunWorkflowResultDto>("run_workflow", {
    workflow_json: JSON.stringify(workflow),
  })
    .then((result) => {
      if (cancelled) {
        return;
      }
      observe({ state: "succeeded", outputs: result.outputs });
    })
    .catch((error: unknown) => {
      if (cancelled) {
        return;
      }
      // Surface the backend error string verbatim rather than swallowing it.
      observe({ state: "failed", reason: String(error) });
    });

  return {
    cancel: () => {
      cancelled = true;
      observe({ state: "failed", reason: "Run cancelled by user" });
    },
  };
}

async function listAssets(kind?: "image" | "video"): Promise<Asset[]> {
  const root = await assetsRoot();
  const assets = await invoke<Asset[]>("list_assets", { kind: kind ?? null });
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

export const tauriApi: WorkflowApi = { runWorkflow, assetsRoot, listAssets, getAsset };
