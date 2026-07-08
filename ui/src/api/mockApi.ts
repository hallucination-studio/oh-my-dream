// Mock backend — an in-browser stand-in implementing the same WorkflowApi as
// the real Tauri client, so the canvas stays fully interactive with no Rust or
// network (e.g. in a plain `vite dev` browser tab). `selectApi` picks this when
// not running inside a Tauri window.

import type { RunOutput, RunOutputs, Workflow } from "../workflow/types.ts";
import type { Asset, RunHandle, RunObserver, WorkflowApi } from "./types.ts";

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
  let cancelled = false;
  const timers: ReturnType<typeof setTimeout>[] = [];

  const outputs: RunOutputs = {};
  workflow.nodes.forEach((node, index) => {
    const timer = setTimeout(
      () => {
        if (cancelled) {
          return;
        }
        observe({ state: "running", nodeId: node.id, progress: 0.5 });
        const produced = outputForNodeType(node.type, node.id);
        if (produced) {
          outputs[node.id] = { [produced.name]: produced.output };
        }
        if (index === workflow.nodes.length - 1) {
          observe({ state: "succeeded", outputs });
        }
      },
      STEP_MS * (index + 1),
    );
    timers.push(timer);
  });

  return {
    cancel: () => {
      cancelled = true;
      timers.forEach(clearTimeout);
      observe({ state: "failed", reason: "Run cancelled by user" });
    },
  };
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

export const mockApi: WorkflowApi = { runWorkflow, assetsRoot, listAssets, getAsset };
