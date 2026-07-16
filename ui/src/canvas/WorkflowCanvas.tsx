import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
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
  onSelectNode: (nodeId: string | null) => void;
  onDrop: (event: React.DragEvent) => void;
}

/** Measured React Flow host that never mounts against a zero-sized container. */
export function WorkflowCanvas(props: Props) {
  const { canvasRef, canvasReady } = useMeasuredCanvas();
  return (
    <div
      ref={canvasRef}
      className="bench__canvas"
      onDrop={props.onDrop}
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
          onNodeClick={(_, node) => props.onSelectNode(node.id)}
          onPaneClick={() => props.onSelectNode(null)}
          defaultEdgeOptions={{ type: "workflow" }}
          fitView
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
