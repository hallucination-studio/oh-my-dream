import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import { useEffect, useRef } from "react";
import { WorkflowFlowNode } from "../nodes/WorkflowFlowNode.tsx";
import { WorkflowEdge } from "../nodes/WorkflowEdge.tsx";
import { useMeasuredCanvas } from "./useMeasuredCanvas.ts";

const nodeTypes = { workflow: WorkflowFlowNode };
const edgeTypes = { workflow: WorkflowEdge };

interface Props {
  nodes: Node[];
  edges: Edge[];
  onNodesChange: (changes: NodeChange<Node>[]) => void;
  onEdgesChange: (changes: EdgeChange<Edge>[]) => void;
  onConnect: (connection: Connection) => void;
  onConnectStart: (nodeId: string, handleId: string) => void;
  onConnectEnd: () => void;
  isValidConnection: (connection: { source: string; sourceHandle?: string | null; target: string; targetHandle?: string | null }) => boolean;
  onSelectNode: (nodeId: string | null) => void;
  onDrop: (event: React.DragEvent, position: { x: number; y: number }) => void;
}

/** Measured React Flow host that never mounts against a zero-sized container. */
export function WorkflowCanvas(props: Props) {
  return (
    <ReactFlowProvider>
      <CanvasInner {...props} />
    </ReactFlowProvider>
  );
}

function CanvasInner(props: Props) {
  const { canvasRef, canvasReady } = useMeasuredCanvas();
  const { screenToFlowPosition, fitView } = useReactFlow();
  const knownIds = useRef<Set<string> | null>(null);

  // Fit the graph after hydration, and bring each newly added node into view.
  useEffect(() => {
    if (props.nodes.length === 0) {
      knownIds.current = null;
      return;
    }
    const ids = new Set(props.nodes.map((node) => node.id));
    const known = knownIds.current;
    if (known === null) {
      knownIds.current = ids;
      void fitView({ padding: 0.12, duration: 200 });
      return;
    }
    if (ids.size === known.size + 1) {
      const added = props.nodes.find((node) => !known.has(node.id));
      if (added) {
        void fitView({ nodes: [{ id: added.id }], padding: 0.3, maxZoom: 1, duration: 200 });
      }
    }
    knownIds.current = ids;
  }, [props.nodes, fitView]);

  return (
    <div
      ref={canvasRef}
      className="bench__canvas"
      onDrop={(event) =>
        props.onDrop(
          event,
          screenToFlowPosition({ x: event.clientX, y: event.clientY }),
        )
      }
      onDragOver={(event) => event.preventDefault()}
    >
      {canvasReady && (
        <ReactFlow
          nodes={props.nodes}
          edges={props.edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onNodesChange={props.onNodesChange}
          onEdgesChange={props.onEdgesChange}
          onConnect={props.onConnect}
          onConnectStart={(_, params) => {
            if (params.nodeId && params.handleId) {
              props.onConnectStart(params.nodeId, params.handleId);
            }
          }}
          onConnectEnd={props.onConnectEnd}
          isValidConnection={props.isValidConnection}
          connectionRadius={22}
          deleteKeyCode={["Backspace", "Delete"]}
          onNodeClick={(_, node) => props.onSelectNode(node.id)}
          onPaneClick={() => props.onSelectNode(null)}
          defaultEdgeOptions={{ type: "workflow" }}
        >
          <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="transparent" />
          <Controls showInteractive={false} />
          <MiniMap
            pannable
            className="bench__minimap"
            bgColor="#f5f6fa"
            maskColor="rgba(20,22,29,0.06)"
            nodeColor="#c3c7d2"
            nodeStrokeWidth={0}
          />
        </ReactFlow>
      )}
    </div>
  );
}
