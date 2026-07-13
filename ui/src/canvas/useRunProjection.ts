// Applies live run status to per-node runtime (state/progress/cost) so the
// canvas nodes reflect execution. Also flags running edges. Kept out of App to
// stay within the file-size budget.

import { useCallback } from "react";
import type { Edge, Node } from "@xyflow/react";
import type { FlowNodeData, NodeRuntime } from "../nodes/WorkflowFlowNode.tsx";
import type { RunProgress, RunTerminalStatus } from "../workflow/types.ts";

export function useRunProjection(
  setNodes: (updater: (nodes: Node[]) => Node[]) => void,
  setEdges: (updater: (edges: Edge[]) => Edge[]) => void,
) {
  const applyProgress = useCallback(
    (progress: RunProgress) => {
      setNodes((current) =>
        current.map((node) =>
          node.id === progress.nodeId
            ? { ...node, data: { ...node.data, runtime: runtimeFor(progress) } }
            : node,
        ),
      );
      setEdges((current) => current.map((edge) => ({
        ...edge,
        data: { ...edge.data, running: true },
      })));
    },
    [setNodes, setEdges],
  );

  const settle = useCallback(
    (status: RunTerminalStatus) => {
      setNodes((current) => current.map((node) => settleNode(node, status)));
      setEdges((current) => current.map((edge) => ({
        ...edge,
        data: { ...edge.data, running: false },
      })));
    },
    [setNodes, setEdges],
  );

  const reset = useCallback(() => {
    setNodes((current) =>
      current.map((n) => ({ ...n, data: { ...n.data, runtime: undefined } })),
    );
    setEdges((current) => current.map((e) => ({ ...e, data: { ...e.data, running: false } })));
  }, [setNodes, setEdges]);

  return { applyProgress, reset, settle };
}

function runtimeFor(progress: RunProgress): NodeRuntime {
  return { state: progress.nodeState, progress: progress.progress, cost: progress.cost };
}

function settleNode(node: Node, status: RunTerminalStatus): Node {
  const data = node.data as FlowNodeData;
  if (status.state === "failed") {
    const rt = data.runtime;
    if (rt?.state === "running") {
      return { ...node, data: { ...data, runtime: { ...rt, state: "error" } } };
    }
    return node;
  }
  if (status.state === "cancelled") {
    return data.runtime?.state === "running"
      ? { ...node, data: { ...data, runtime: undefined } }
      : node;
  }
  if (status.state !== "succeeded") {
    return node;
  }
  // Succeeded: any node that produced an output is done, with a preview.
  const outputs = status.outputs[node.id];
  if (!outputs) {
    return data.runtime?.state === "running"
      ? { ...node, data: { ...data, runtime: { ...data.runtime, state: "done" } } }
      : node;
  }
  const first = Object.values(outputs)[0];
  const preview =
    first && (first.kind === "image" || first.kind === "video" || first.kind === "audio")
      ? { kind: first.kind, url: mediaUrl(first.value) }
      : undefined;
  return {
    ...node,
    data: { ...data, runtime: { state: "done", progress: 1, cost: data.runtime?.cost, preview } },
  };
}

function mediaUrl(value: string): string | null {
  // Mock outputs are opaque refs (mock://…); real backends return asset URLs.
  return value.startsWith("mock://") ? null : value;
}
