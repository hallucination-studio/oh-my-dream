// Mock backend API — the single seam that becomes real Tauri commands in Wave 2.
//
// Today it simulates "submit workflow -> progress per node -> produced outputs"
// entirely in the browser, so the canvas is fully interactive with no Rust or
// network. Swap only this file's implementation later; callers stay unchanged.

import type { RunOutput, RunStatus, Workflow } from "../workflow/types.ts";

/** Callback invoked with each status transition during a mock run. */
export type RunObserver = (status: RunStatus) => void;

/** A handle allowing the caller to cancel an in-flight mock run. */
export interface RunHandle {
  cancel: () => void;
}

const STEP_MS = 400;

function outputForNodeType(type: string, nodeId: string): RunOutput | null {
  switch (type) {
    case "TextToImage":
      return { kind: "image", value: `mock://image/${nodeId}` };
    case "ImageToVideo":
      return { kind: "video", value: `mock://video/${nodeId}` };
    case "TextPrompt":
      return { kind: "string", value: `mock://text/${nodeId}` };
    default:
      return null;
  }
}

/**
 * Simulates executing `workflow`, emitting running/succeeded/failed transitions
 * through `observe`. Nodes run in array order (a stand-in for the engine's
 * topological order until the real command is wired in).
 */
export function runWorkflowMock(
  workflow: Workflow,
  observe: RunObserver,
): RunHandle {
  let cancelled = false;
  const timers: ReturnType<typeof setTimeout>[] = [];

  const outputs: Record<string, RunOutput> = {};
  workflow.nodes.forEach((node, index) => {
    const timer = setTimeout(() => {
      if (cancelled) {
        return;
      }
      observe({ state: "running", nodeId: node.id, progress: 0.5 });
      const produced = outputForNodeType(node.type, node.id);
      if (produced) {
        outputs[node.id] = produced;
      }
      if (index === workflow.nodes.length - 1) {
        observe({ state: "succeeded", outputs });
      }
    }, STEP_MS * (index + 1));
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
