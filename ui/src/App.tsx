import { useCallback, useMemo, useRef, useState } from "react";
import {
  addEdge,
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
} from "@xyflow/react";
import { NODE_TYPES, findNodeType } from "./nodes/catalog.ts";
import { WorkflowFlowNode, type FlowNodeData } from "./nodes/WorkflowFlowNode.tsx";
import { WorkflowEdge } from "./nodes/WorkflowEdge.tsx";
import { typeColor } from "./nodes/typeColor.ts";
import { toWorkflow } from "./workflow/serialize.ts";
import { isValidConnection } from "./workflow/validate.ts";
import { api, type RunHandle } from "./api/index.ts";
import type { RunStatus } from "./workflow/types.ts";
import { TopBar } from "./components/TopBar.tsx";
import { NodePalette } from "./components/NodePalette.tsx";
import { InspectorPanel, type SelectedNode } from "./components/InspectorPanel.tsx";
import { AssetDrawer } from "./assets/AssetDrawer.tsx";
import { useAssets } from "./assets/useAssets.ts";

let idCounter = 0;
const nextId = () => `n${++idCounter}`;

const nodeTypes = { workflow: WorkflowFlowNode };
const edgeTypes = { workflow: WorkflowEdge };

export function App() {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [status, setStatus] = useState<RunStatus>({ state: "idle" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const runHandle = useRef<RunHandle | null>(null);
  const { assets, error: assetError, refresh } = useAssets();

  const setParam = useCallback(
    (nodeId: string, name: string, value: unknown) => {
      setNodes((current) =>
        current.map((node) =>
          node.id === nodeId
            ? {
                ...node,
                data: {
                  ...node.data,
                  params: { ...(node.data as FlowNodeData).params, [name]: value },
                },
              }
            : node,
        ),
      );
    },
    [setNodes],
  );

  const addNode = useCallback(
    (type: string) => {
      const spec = NODE_TYPES.find((s) => s.type === type);
      if (!spec) {
        return;
      }
      const id = nextId();
      const params = Object.fromEntries(spec.params.map((p) => [p.name, p.default]));
      setNodes((current) => [
        ...current,
        {
          id,
          type: "workflow",
          position: { x: 120 + current.length * 60, y: 90 + current.length * 40 },
          data: {
            type,
            params,
            onParamChange: (n: string, v: unknown) => setParam(id, n, v),
          } as FlowNodeData,
        },
      ]);
    },
    [setNodes, setParam],
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      if (!isValidConnection(connection, nodes)) {
        setStatus({ state: "failed", reason: "Incompatible port types — connection rejected" });
        return;
      }
      const source = nodes.find((n) => n.id === connection.source);
      const spec = source ? findNodeType((source.data as FlowNodeData).type) : undefined;
      const outType = spec?.outputs.find((p) => p.name === connection.sourceHandle)?.type;
      setEdges((current) =>
        addEdge(
          { ...connection, type: "workflow", data: { color: typeColor(outType) } },
          current,
        ),
      );
    },
    [nodes, setEdges],
  );

  const selected: SelectedNode | null = useMemo(() => {
    const node = nodes.find((n) => n.id === selectedId);
    if (!node) {
      return null;
    }
    const data = node.data as FlowNodeData;
    return { id: node.id, type: data.type, params: data.params };
  }, [nodes, selectedId]);

  const markRunning = useCallback(
    (nodeId: string) => {
      setNodes((current) =>
        current.map((n) => ({ ...n, data: { ...n.data, running: n.id === nodeId } })),
      );
      setEdges((current) => current.map((e) => ({ ...e, data: { ...e.data, running: true } })));
    },
    [setNodes, setEdges],
  );

  const settle = useCallback(() => {
    setNodes((current) =>
      current.map((n) => ({ ...n, data: { ...n.data, running: false, done: true } })),
    );
    setEdges((current) => current.map((e) => ({ ...e, data: { ...e.data, running: false } })));
  }, [setNodes, setEdges]);

  const run = useCallback(() => {
    const workflow = toWorkflow(nodes, edges);
    const observe = (next: RunStatus) => {
      setStatus(next);
      if (next.state === "running") {
        markRunning(next.nodeId);
      } else if (next.state === "succeeded") {
        settle();
        void refresh();
      } else if (next.state === "failed") {
        settle();
      }
    };
    setStatus({ state: "running", nodeId: workflow.nodes[0]?.id ?? "", progress: 0 });
    runHandle.current = api.runWorkflow(workflow, observe);
  }, [nodes, edges, markRunning, settle, refresh]);

  const cancel = useCallback(() => runHandle.current?.cancel(), []);

  return (
    <div className="bench">
      <TopBar status={status} onRun={run} onCancel={cancel} />
      <div className="bench__body">
        <NodePalette onAdd={addNode} />
        <div className="bench__canvas">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={(_, node) => setSelectedId(node.id)}
            onPaneClick={() => setSelectedId(null)}
            defaultEdgeOptions={{ type: "workflow" }}
            proOptions={{ hideAttribution: false }}
            fitView
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="transparent" />
            <Controls />
          </ReactFlow>
        </div>
        <InspectorPanel node={selected} onParamChange={setParam} />
      </div>
      <AssetDrawer assets={assets} error={assetError} />
    </div>
  );
}
