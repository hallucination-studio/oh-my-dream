import { useCallback, useEffect, useMemo, useState } from "react";
import {
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
import { nextNodeId, upsertIncomingEdge } from "./workflow/editor.ts";
import { toWorkflow } from "./workflow/serialize.ts";
import { isValidConnection } from "./workflow/validate.ts";
import { api, type Project } from "./api/index.ts";
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
import { useProjectWorkspace } from "./workflow/useProjectWorkspace.ts";
import { useRunController } from "./workflow/useRunController.ts";
import { AssistantDock } from "./assistant/AssistantDock.tsx";
import { createCapabilityExecutor, type CanvasActions } from "./assistant/capabilityExecutor.ts";

const nodeTypes = { workflow: WorkflowFlowNode };
const edgeTypes = { workflow: WorkflowEdge };

function isGraphMutation(change: { type: string }): boolean {
  return change.type === "add" || change.type === "remove" || change.type === "replace" || change.type === "position";
}

export function App() {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [status, setStatus] = useState<RunStatus>({ state: "idle" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState<RailTab>("nodes");
  const [project, setProject] = useState<Project | null>(null);
  const [projectsOpen, setProjectsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [assistantEnabled, setAssistantEnabled] = useState(false);
  const { assets, error: assetError, refresh } = useAssets();
  const { applyProgress, reset: resetRun, settle } = useRunProjection(setNodes, setEdges);
  const { cancel, invalidateRun, run } = useRunController({
    nodes,
    edges,
    projectId: project?.id ?? null,
    setStatus,
    resetProjection: resetRun,
    applyProgress,
    settleProjection: settle,
    onSucceeded: () => void refresh(),
  });
  const { markWorkflowMutation, openProject, setParam } = useProjectWorkspace({
    project,
    setProject,
    nodes,
    edges,
    setNodes,
    setEdges,
    setSelectedId,
    setProjectsOpen,
    setStatus,
    invalidateRun,
  });

  // Reflect the assistant enable flag: the rail button and dock only appear
  // when the assistant is enabled in settings. Re-read after settings closes.
  const refreshAssistantEnabled = useCallback(() => {
    void api
      .getAssistantConfig()
      .then((config) => setAssistantEnabled(config.enabled))
      .catch(() => setAssistantEnabled(false));
  }, []);
  useEffect(refreshAssistantEnabled, [refreshAssistantEnabled]);
  useEffect(() => {
    if (!assistantEnabled && assistantOpen) {
      setAssistantOpen(false);
    }
  }, [assistantEnabled, assistantOpen]);

  const addNode = useCallback(
    (type: string, position?: { x: number; y: number }) => {
      const spec = NODE_TYPES.find((s) => s.type === type);
      if (!spec) {
        return;
      }
      const params = Object.fromEntries(spec.params.map((p) => [p.name, p.default]));
      markWorkflowMutation();
      setNodes((current) => {
        const id = nextNodeId(current);
        return [
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
        ];
      });
    },
    [markWorkflowMutation, setNodes, setParam],
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
      markWorkflowMutation();
      setEdges((current) =>
        upsertIncomingEdge(current, connection, { color: typeColor(outType) }),
      );
    },
    [markWorkflowMutation, nodes, setEdges],
  );

  const connectNodes = useCallback(
    (args: Parameters<CanvasActions["connectNodes"]>[0]) => {
      onConnect({
        source: args.source_node_id,
        sourceHandle: args.source_output,
        target: args.target_node_id,
        targetHandle: args.target_input,
      });
    },
    [onConnect],
  );

  const deleteNode = useCallback(
    (nodeId: string) => {
      markWorkflowMutation();
      setNodes((current) => current.filter((node) => node.id !== nodeId));
      setEdges((current) => current.filter((edge) => edge.source !== nodeId && edge.target !== nodeId));
      setSelectedId((current) => (current === nodeId ? null : current));
    },
    [markWorkflowMutation, setEdges, setNodes],
  );

  const handleNodesChange = useCallback(
    (changes: Parameters<typeof onNodesChange>[0]) => {
      if (changes.some(isGraphMutation)) {
        markWorkflowMutation();
      }
      onNodesChange(changes);
    },
    [markWorkflowMutation, onNodesChange],
  );

  const handleEdgesChange = useCallback(
    (changes: Parameters<typeof onEdgesChange>[0]) => {
      if (changes.some(isGraphMutation)) {
        markWorkflowMutation();
      }
      onEdgesChange(changes);
    },
    [markWorkflowMutation, onEdgesChange],
  );

  const selected: SelectedNode | null = useMemo(() => {
    const node = nodes.find((n) => n.id === selectedId);
    if (!node) {
      return null;
    }
    const data = node.data as FlowNodeData;
    return { id: node.id, type: data.type, params: data.params };
  }, [nodes, selectedId]);

  const assistantActions = useMemo<CanvasActions>(
    () => ({
      addNode,
      connectNodes,
      setParam,
      deleteNode,
      selectNode: setSelectedId,
      switchTab: setTab,
      getCanvasState: () => toWorkflow(nodes, edges, project?.id ?? "default"),
      getSelection: () => selected,
    }),
    [addNode, connectNodes, deleteNode, edges, nodes, project, selected, setParam],
  );
  const assistantExecutor = useMemo(
    () => createCapabilityExecutor({ actions: assistantActions, api }),
    [assistantActions],
  );
  const assistantContext = useCallback(
    () => ({
      project_id: project?.id ?? "default",
      canvas_snapshot: toWorkflow(nodes, edges, project?.id ?? "default"),
      selection: selected,
    }),
    [edges, nodes, project, selected],
  );

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

      <main
        className={`bench__body${tab === "assets" ? " bench__body--assets" : ""}${
          assistantOpen ? " bench__body--assistant" : ""
        }`}
      >
        <IconRail
          tab={tab}
          assistantEnabled={assistantEnabled}
          assistantOpen={assistantOpen}
          onSelect={setTab}
          onToggleAssistant={() => setAssistantOpen((open) => !open)}
        />
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
                onNodesChange={handleNodesChange}
                onEdgesChange={handleEdgesChange}
                onConnect={onConnect}
                onNodeClick={(_, node) => setSelectedId(node.id)}
                onPaneClick={() => setSelectedId(null)}
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
            </div>
            <InspectorPanel node={selected} onParamChange={setParam} />
          </>
        )}
        {assistantEnabled && assistantOpen && (
          <AssistantDock
            onClose={() => setAssistantOpen(false)}
            executeCapability={assistantExecutor.execute}
            getContext={assistantContext}
          />
        )}
      </main>

      <SettingsDialog
        open={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
          refreshAssistantEnabled();
        }}
      />
    </div>
  );
}
