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
  type ReactFlowInstance,
} from "@xyflow/react";
import { useEffect, useRef } from "react";
import { WorkflowFlowNode } from "../nodes/WorkflowFlowNode.tsx";
import { WorkflowEdge } from "../nodes/WorkflowEdge.tsx";
import { useMeasuredCanvas } from "./useMeasuredCanvas.ts";
import { MINIMAP_THEME } from "./minimapTheme.ts";

const nodeTypes = { workflow: WorkflowFlowNode };
const edgeTypes = { workflow: WorkflowEdge };

/** Screen-space margins the overlays carve out of the canvas stage. */
export interface CanvasInsets {
  left: number;
  right: number;
}

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
  focusNodeId?: string | null;
  projectId?: string | null;
  insets?: CanvasInsets;
  onInit?: (instance: ReactFlowInstance) => void;
  children?: React.ReactNode;
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
  const instance = useReactFlow();
  const { screenToFlowPosition, flowToScreenPosition, setViewport } = instance;
  const knownIds = useRef<Set<string> | null>(null);
  const focusedRef = useRef<string | null>(null);
  const insets = props.insets ?? { left: 0, right: 0 };
  const insetsRef = useRef(insets);
  insetsRef.current = insets;

  // Project switches reset the fit bookkeeping so the next graph gets a fresh fit.
  useEffect(() => {
    knownIds.current = null;
    focusedRef.current = null;
  }, [props.projectId]);

  /** The visible canvas rect: the stage box minus the overlay insets and a margin. */
  const visibleRect = () => {
    const box = canvasRef.current?.getBoundingClientRect();
    if (!box) return null;
    const { left, right } = insetsRef.current;
    return {
      left: box.left + left + 16,
      right: box.right - right - 16,
      top: box.top + 16,
      bottom: box.bottom - 16,
    };
  };

  /** Center one node inside the visible rect without changing zoom. */
  const centerNode = (nodeId: string, duration: number) => {
    const rect = visibleRect();
    const node = instance.getNode(nodeId);
    if (!rect || !node) return;
    const width = node.measured?.width ?? node.width ?? 336;
    const height = node.measured?.height ?? node.height ?? 360;
    const zoom = Math.min(instance.getViewport().zoom, 1);
    const cx = node.position.x + width / 2;
    const cy = node.position.y + height / 2;
    void setViewport(
      {
        x: (rect.left + rect.right) / 2 - cx * zoom,
        y: (rect.top + rect.bottom) / 2 - cy * zoom,
        zoom,
      },
      { duration },
    );
  };

  /** True when the node's full bounds already sit inside the visible rect. */
  const nodeFullyVisible = (nodeId: string): boolean => {
    const rect = visibleRect();
    const node = instance.getNode(nodeId);
    if (!rect || !node) return true;
    const width = node.measured?.width ?? node.width ?? 336;
    const height = node.measured?.height ?? node.height ?? 360;
    const tl = flowToScreenPosition(node.position);
    const br = flowToScreenPosition({ x: node.position.x + width, y: node.position.y + height });
    return tl.x >= rect.left && br.x <= rect.right && tl.y >= rect.top && br.y <= rect.bottom;
  };

  // Bring an externally requested node (for example an Asset's origin) into view.
  useEffect(() => {
    if (!props.focusNodeId || props.focusNodeId === focusedRef.current) return;
    focusedRef.current = props.focusNodeId;
    centerNode(props.focusNodeId, 250);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.focusNodeId]);

  // Fit the graph after hydration; bring a newly added node into view only when
  // it actually landed outside the visible rect (never yank the viewport).
  useEffect(() => {
    if (props.nodes.length === 0) {
      knownIds.current = null;
      return;
    }
    const ids = new Set(props.nodes.map((node) => node.id));
    const known = knownIds.current;
    if (known === null) {
      knownIds.current = ids;
      void instance.fitView({ padding: 0.12, duration: 200 });
      return;
    }
    if (ids.size === known.size + 1) {
      const added = props.nodes.find((node) => !known.has(node.id));
      if (added && !nodeFullyVisible(added.id)) {
        centerNode(added.id, 200);
      }
    }
    knownIds.current = ids;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.nodes]);

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
          onInit={props.onInit}
          defaultEdgeOptions={{ type: "workflow" }}
        >
          <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="transparent" />
          <Controls showInteractive={false} position="bottom-left" style={{ left: insets.left + 12 }} />
          <MiniMap
            pannable
            position="bottom-left"
            style={{ left: insets.left + 12 }}
            className="bench__minimap"
            bgColor={MINIMAP_THEME.bg}
            maskColor={MINIMAP_THEME.mask}
            nodeColor={MINIMAP_THEME.node}
            nodeStrokeWidth={0}
          />
        </ReactFlow>
      )}
      {props.children}
    </div>
  );
}
