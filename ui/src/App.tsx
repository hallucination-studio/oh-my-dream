import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addEdge,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
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
import { api, type Project, type RunHandle } from "./api/index.ts";
import type { RunStatus } from "./workflow/types.ts";
import { TopBar } from "./components/TopBar.tsx";
import { IconRail, type RailTab } from "./components/IconRail.tsx";
import { NodeLibrary } from "./components/NodeLibrary.tsx";
import { InspectorPanel, type SelectedNode } from "./components/InspectorPanel.tsx";
import { ProjectSwitcher } from "./components/ProjectSwitcher.tsx";
import { SettingsDialog } from "./components/SettingsDialog.tsx";
import { AssetLibrary } from "./assets/AssetLibrary.tsx";
import { useAssets } from "./assets/useAssets.ts";
import { useRunProjection } from "./canvas/useRunProjection.ts";

let idCounter = 0;
const nextId = () => `n${++idCounter}`;

const nodeTypes = { workflow: WorkflowFlowNode };
const edgeTypes = { workflow: WorkflowEdge };

export function App() {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [status, setStatus] = useState<RunStatus>({ state: "idle" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState<RailTab>("nodes");
  const [project, setProject] = useState<Project | null>(null);
  const [projectsOpen, setProjectsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const runHandle = useRef<RunHandle | null>(null);
  const { assets, error: assetError, refresh } = useAssets();
  const { apply: projectRun, reset: resetRun } = useRunProjection(setNodes, setEdges);

  // Load (or create) an initial project on mount.
  useEffect(() => {
    api.listProjects().then((list) => {
      const first = list[0];
      if (first) {
        setProject(first);
      }
    });
  }, []);

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
    (type: string, position?: { x: number; y: number }) => {
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
          position: position ?? { x: 140 + current.length * 60, y: 100 + current.length * 40 },
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
        addEdge({ ...connection, type: "workflow", data: { color: typeColor(outType) } }, current),
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

  const run = useCallback(() => {
    const workflow = toWorkflow(nodes, edges, project?.id ?? "default");
    resetRun();
    const observe = (next: RunStatus) => {
      setStatus(next);
      projectRun(next);
      if (next.state === "succeeded") {
        void refresh();
      }
    };
    setStatus({ state: "running", nodeId: workflow.nodes[0]?.id ?? "", progress: 0 });
    runHandle.current = api.runWorkflow(workflow, observe);
  }, [nodes, edges, project, resetRun, projectRun, refresh]);

  const cancel = useCallback(() => runHandle.current?.cancel(), []);

  const openProject = useCallback((id: string) => {
    setProjectsOpen(false);
    api.openProject(id).then((ws) => setProject(ws.project));
  }, []);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const type = e.dataTransfer.getData("application/oh-node");
      if (type) {
        addNode(type, { x: e.clientX - 320, y: e.clientY - 90 });
      }
    },
    [addNode],
  );

  return (
    <div className="bench">
      <TopBar
        project={project}
        status={status}
        onOpenProjects={() => setProjectsOpen((v) => !v)}
        onOpenSettings={() => setSettingsOpen(true)}
        onRun={run}
        onCancel={cancel}
      />
      <ProjectSwitcher
        current={project}
        open={projectsOpen}
        onClose={() => setProjectsOpen(false)}
        onOpenProject={openProject}
      />

      <div className={`bench__body${tab === "assets" ? " bench__body--assets" : ""}`}>
        <IconRail tab={tab} onSelect={setTab} />
        {tab === "nodes" ? (
          <NodeLibrary onAdd={(type) => addNode(type)} />
        ) : (
          <AssetLibrary
            assets={assets}
            error={assetError}
            onAddToCanvas={() => setTab("nodes")}
            onJumpToNode={() => setTab("nodes")}
          />
        )}

        {tab === "nodes" && (
          <>
            <div className="bench__canvas" onDrop={onDrop} onDragOver={(e) => e.preventDefault()}>
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
                fitView
              >
                <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="transparent" />
                <Controls showInteractive={false} />
                <MiniMap pannable className="bench__minimap" />
              </ReactFlow>
            </div>
            <InspectorPanel node={selected} onParamChange={setParam} />
          </>
        )}
      </div>

      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
