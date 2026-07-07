import { useCallback, useMemo, useRef, useState } from "react";
import {
  addEdge,
  Background,
  Controls,
  ReactFlow,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
} from "@xyflow/react";
import { NODE_TYPES } from "./nodes/catalog.ts";
import { WorkflowFlowNode, type FlowNodeData } from "./nodes/WorkflowFlowNode.tsx";
import { toWorkflow } from "./workflow/serialize.ts";
import { api, type RunHandle } from "./api/index.ts";
import type { RunStatus } from "./workflow/types.ts";
import { isValidConnection } from "./workflow/validate.ts";

let idCounter = 0;
const nextId = () => `n${++idCounter}`;

export function App() {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [status, setStatus] = useState<RunStatus>({ state: "idle" });
  const runHandle = useRef<RunHandle | null>(null);

  const nodeTypes = useMemo(() => ({ workflow: WorkflowFlowNode }), []);

  const setParam = useCallback(
    (nodeId: string, name: string, value: unknown) => {
      setNodes((current) =>
        current.map((node) =>
          node.id === nodeId
            ? { ...node, data: { ...node.data, params: { ...(node.data as FlowNodeData).params, [name]: value } } }
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
          position: { x: 80 + current.length * 40, y: 80 + current.length * 30 },
          data: { type, params, onParamChange: (n: string, v: unknown) => setParam(id, n, v) } as FlowNodeData,
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
      setEdges((current) => addEdge(connection, current));
    },
    [nodes, setEdges],
  );

  const run = useCallback(() => {
    const workflow = toWorkflow(nodes, edges);
    setStatus({ state: "running", nodeId: workflow.nodes[0]?.id ?? "", progress: 0 });
    runHandle.current = api.runWorkflow(workflow, setStatus);
  }, [nodes, edges]);

  const cancel = useCallback(() => runHandle.current?.cancel(), []);

  return (
    <div className="app">
      <aside className="palette">
        <h2>Nodes</h2>
        {NODE_TYPES.map((spec) => (
          <button key={spec.type} onClick={() => addNode(spec.type)}>
            + {spec.label}
          </button>
        ))}
        <div className="palette__run">
          <button className="run" onClick={run}>Run</button>
          <button onClick={cancel}>Cancel</button>
          <StatusBar status={status} />
        </div>
      </aside>
      <div className="canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          fitView
        >
          <Background />
          <Controls />
        </ReactFlow>
      </div>
    </div>
  );
}

function StatusBar({ status }: { status: RunStatus }) {
  switch (status.state) {
    case "idle":
      return <p className="status">Idle</p>;
    case "running":
      return <p className="status status--running">Running {status.nodeId}…</p>;
    case "succeeded":
      return <p className="status status--ok">Done — {Object.keys(status.outputs).length} outputs</p>;
    case "failed":
      return <p className="status status--err">{status.reason}</p>;
  }
}
