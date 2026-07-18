import { act, renderHook } from "@testing-library/react";
import type { Edge, Node } from "@xyflow/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { useRunProjection } from "./useRunProjection.ts";

describe("useRunProjection", () => {
  it("clears only the active node after authoritative cancellation", () => {
    const initialNodes: Node[] = [
      nodeWithState("completed", "done"),
      nodeWithState("active", "running"),
    ];
    const initialEdges: Edge[] = [
      { id: "completed-active", source: "completed", target: "active", data: { running: true } },
    ];

    const view = renderHook(() => {
      const [nodes, setNodes] = useState(initialNodes);
      const [edges, setEdges] = useState(initialEdges);
      const projection = useRunProjection(setNodes, setEdges);
      return { nodes, edges, projection };
    });

    act(() => view.result.current.projection.settle({ state: "cancelled" }));

    expect(runtimeOf(view.result.current.nodes, "completed")?.state).toBe("done");
    expect(runtimeOf(view.result.current.nodes, "active")).toBeUndefined();
    expect(view.result.current.edges[0]?.data?.running).toBe(false);
  });

  it("selects a media preview even when a non-media output comes first", () => {
    const view = renderHook(() => {
      const [nodes, setNodes] = useState([nodeWithState("result", "running")]);
      const [, setEdges] = useState<Edge[]>([]);
      const projection = useRunProjection(setNodes, setEdges);
      return { nodes, projection };
    });

    act(() => view.result.current.projection.settle({
      state: "succeeded",
      steps: 1,
      outputs: {
        result: {
          description: { kind: "string", value: "finished" },
          image: { kind: "image", value: "desktop-asset://v1/token" },
        },
      },
    }));

    expect(runtimeOf(view.result.current.nodes, "result")?.preview).toEqual({
      kind: "image",
      url: "desktop-asset://v1/token",
    });
  });

  it("never renders opaque refs or raw asset ids as previews", () => {
    const view = renderHook(() => {
      const [nodes, setNodes] = useState([nodeWithState("result", "running")]);
      const [, setEdges] = useState<Edge[]>([]);
      const projection = useRunProjection(setNodes, setEdges);
      return { nodes, projection };
    });

    act(() => view.result.current.projection.settle({
      state: "succeeded",
      steps: 1,
      outputs: {
        result: { image: { kind: "image", value: "mock-asset-image-001" } },
      },
    }));

    expect(runtimeOf(view.result.current.nodes, "result")?.preview).toEqual({
      kind: "image",
      url: null,
    });
  });
});

function nodeWithState(id: string, state: "done" | "running"): Node {
  return {
    id,
    type: "workflow",
    position: { x: 0, y: 0 },
    data: {
      type: "TextPrompt",
      params: {},
      runtime: { state },
      onParamChange: () => {},
    } satisfies FlowNodeData,
  };
}

function runtimeOf(nodes: Node[], id: string) {
  return (nodes.find((node) => node.id === id)?.data as FlowNodeData | undefined)?.runtime;
}
