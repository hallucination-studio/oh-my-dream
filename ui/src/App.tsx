import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import {
  nodeSpecFromBundle,
  nodeSpecsFromSnapshot,
  paramsForMode,
  recoveryNodeSpec,
} from "./nodes/catalog.ts";
import { WorkflowFlowNode, type FlowNodeData } from "./nodes/WorkflowFlowNode.tsx";
import { WorkflowEdge } from "./nodes/WorkflowEdge.tsx";
import { typeColor } from "./nodes/typeColor.ts";
import { nextNodeId, upsertIncomingEdge } from "./workflow/editor.ts";
import { isValidConnection } from "./workflow/validate.ts";
import { api, type CapabilityRef, type Project } from "./api/index.ts";
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
import { CloseFailureDialog } from "./components/CloseFailureDialog.tsx";
import { CapabilityContractCache, useCapabilityCache } from "./workflow/contractCache.ts";
import type { CapabilityCacheSnapshot } from "./workflow/contractCache.ts";

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
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(null);
  const capabilityCacheRef = useRef<CapabilityContractCache | null>(null);
  if (capabilityCacheRef.current === null) {
    capabilityCacheRef.current = new CapabilityContractCache(api);
  }
  const capabilityCache = capabilityCacheRef.current;
  const capabilitySnapshot = useCapabilityCache(capabilityCache);
  const { assets, error: assetError, refresh } = useAssets();
  const { applyProgress, reset: resetRun, settle } = useRunProjection(setNodes, setEdges);
  const { cancel, invalidateRun, run: runWorkflow } = useRunController({
    nodes,
    edges,
    projectId: project?.id ?? null,
    setStatus,
    resetProjection: resetRun,
    applyProgress,
    settleProjection: settle,
    onSucceeded: () => void refresh(),
  });
  const {
    canEdit,
    markWorkflowMutation,
    openProject,
    runAfterBarrier,
    closeError,
    discardAndClose,
    keepEditing,
    setParam,
    replaceParams,
    adoptWorkflowHead,
    workspaceState,
  } = useProjectWorkspace({
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
    capabilityCache,
  });
  const run = useCallback(() => {
    void runAfterBarrier("prepare_run", runWorkflow).catch((error: unknown) => {
      setStatus({ state: "failed", reason: String(error) });
    });
  }, [runAfterBarrier, runWorkflow]);

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
    (requested: string | CapabilityRef, position?: { x: number; y: number }, contextualParams?: Record<string, unknown>) => {
      if (!canEdit) {
        return;
      }
      const reference = resolveRequestedRef(requested, capabilitySnapshot);
      void capabilityCache.load([reference]).then(() => {
        if (!canEdit) return;
        const bundle = capabilityCache.get(reference);
        const spec = bundle
          ? nodeSpecFromBundle(bundle)
          : recoveryNodeSpec(reference, "exact capability bundle is unavailable");
        if (spec.contract === null || spec.status.availability !== "available") return;
        if (spec.contextualCreationRoute !== null && contextualParams === undefined) return;
        const params = contextualParams ?? Object.fromEntries(spec.params.map((param) => [param.name, param.default]));
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
                type: reference.id,
                contractVersion: reference.version,
                capability: spec,
                params,
                onParamChange: (name: string, value: unknown) => setParam(id, name, value),
              } satisfies FlowNodeData,
            },
          ];
        });
      }).catch((error: unknown) => {
        setStatus({ state: "failed", reason: String(error) });
      });
    },
    [canEdit, capabilityCache, capabilitySnapshot, markWorkflowMutation, setNodes, setParam, setStatus],
  );

  const addAssetSource = useCallback((assetId: string, position?: { x: number; y: number }) => {
    const asset = assets.find((candidate) => candidate.id === assetId);
    if (!asset) {
      setStatus({ state: "failed", reason: "Asset is unavailable in the current workspace" });
      return;
    }
    const summary = capabilitySnapshot.summaries.find((candidate) =>
      candidate.selector.type_id.toLowerCase() === asset.kind && candidate.selector.mode === "asset"
    );
    if (!summary) {
      setStatus({ state: "failed", reason: `No ${asset.kind} Asset Source capability is available` });
      return;
    }
    addNode(summary.reference, position, { mode: "asset", asset_id: asset.id });
    setSelectedAssetId(asset.id);
  }, [addNode, assets, capabilitySnapshot.summaries]);

  useEffect(() => {
    const assetIndex = new Map(assets.map((asset) => [asset.id, asset]));
    setNodes((current) => current.map((node) => {
      const data = node.data as FlowNodeData;
      if (data.capability?.selector?.mode !== "asset") return node;
      const assetId = typeof data.params.asset_id === "string" ? data.params.asset_id : "";
      const asset = assetIndex.get(assetId);
      return {
        ...node,
        data: {
          ...data,
          runtime: asset
            ? { ...data.runtime, state: data.runtime?.state ?? "idle", preview: { kind: asset.kind, url: asset.fileUrl } }
            : { ...data.runtime, state: data.runtime?.state ?? "idle", preview: { kind: data.capability.selector?.type_id.toLowerCase() as "image" | "video" | "audio", url: null } },
        },
      };
    }));
  }, [assets, setNodes]);

  const onConnect = useCallback(
    (connection: Connection) => {
      if (!canEdit) {
        return;
      }
      if (!isValidConnection(connection, nodes)) {
        setStatus({ state: "failed", reason: "Incompatible port types — connection rejected" });
        return;
      }
      const source = nodes.find((n) => n.id === connection.source);
      const target = nodes.find((n) => n.id === connection.target);
      const spec = source ? (source.data as FlowNodeData).capability : undefined;
      const targetSpec = target ? (target.data as FlowNodeData).capability : undefined;
      const outType = spec?.outputs.find((p) => p.name === connection.sourceHandle)?.type;
      const targetCardinality = targetSpec?.inputs.find(
        (port) => port.name === connection.targetHandle,
      )?.cardinality;
      markWorkflowMutation();
      setEdges((current) =>
        upsertIncomingEdge(
          current,
          connection,
          { color: typeColor(outType) },
          targetCardinality !== undefined && targetCardinality !== "one",
        ),
      );
    },
    [canEdit, markWorkflowMutation, nodes, setEdges],
  );

  const handleNodesChange = useCallback(
    (changes: Parameters<typeof onNodesChange>[0]) => {
      if (!canEdit) {
        return;
      }
      if (changes.some(isGraphMutation)) {
        markWorkflowMutation();
      }
      onNodesChange(changes);
    },
    [canEdit, markWorkflowMutation, onNodesChange],
  );

  const handleEdgesChange = useCallback(
    (changes: Parameters<typeof onEdgesChange>[0]) => {
      if (!canEdit) {
        return;
      }
      if (changes.some(isGraphMutation)) {
        markWorkflowMutation();
      }
      onEdgesChange(changes);
    },
    [canEdit, markWorkflowMutation, onEdgesChange],
  );

  const selected: SelectedNode | null = useMemo(() => {
    const node = nodes.find((n) => n.id === selectedId);
    if (!node) {
      return null;
    }
    const data = node.data as FlowNodeData;
    return { id: node.id, type: data.type, params: data.params, capability: data.capability };
  }, [nodes, selectedId]);
  const [modeOptions, setModeOptions] = useState<ReturnType<typeof nodeSpecFromBundle>[]>([]);
  useEffect(() => {
    const typeId = selected?.capability?.selector?.type_id;
    if (!typeId) {
      setModeOptions([]);
      return;
    }
    let cancelled = false;
    void capabilityCache.loadModes(typeId).then((summaries) => {
      if (cancelled) return;
      setModeOptions(summaries.flatMap((summary) => {
        const bundle = capabilityCache.get(summary.reference);
        return bundle ? [nodeSpecFromBundle(bundle)] : [];
      }));
    }).catch((error: unknown) => {
      if (!cancelled) setStatus({ state: "failed", reason: String(error) });
    });
    return () => { cancelled = true; };
  }, [capabilityCache, selected?.capability?.selector?.type_id, setStatus]);

  const changeMode = useCallback(async (mode: string) => {
    if (!selected) return;
    const spec = modeOptions.find((candidate) => candidate.selector?.mode === mode);
    if (!spec) return;
    try {
      await replaceParams(selected.id, paramsForMode(spec, selected.params));
    } catch (error: unknown) {
      setStatus({ state: "failed", reason: String(error) });
    }
  }, [modeOptions, replaceParams, selected, setStatus]);

  const assistantContext = useCallback(
    () => ({
      project_id: project?.id ?? null,
      workflow_present: workspaceState.state === "ready" && workspaceState.workflowHead !== null,
      workflow_revision:
        workspaceState.state === "ready" ? workspaceState.workflowHead?.revision ?? null : null,
      selected_node_ids: selected ? [selected.id] : [],
      selected_asset_ids: selectedAssetId ? [selectedAssetId] : [],
    }),
    [project, selected, selectedAssetId, workspaceState],
  );

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const encoded = e.dataTransfer.getData("application/oh-node");
      if (encoded) {
        addNode(parseCapabilityRef(encoded), { x: e.clientX - 320, y: e.clientY - 90 });
        return;
      }
      const assetId = e.dataTransfer.getData("application/oh-asset");
      if (assetId) addAssetSource(assetId, { x: e.clientX - 320, y: e.clientY - 90 });
    },
    [addAssetSource, addNode],
  );
  const searchCapabilities = useCallback(
    (request: Parameters<CapabilityContractCache["search"]>[0]) => capabilityCache.search(request),
    [capabilityCache],
  );

  return (
    <div className="bench">
      <TopBar
        project={project}
        status={status}
        workspaceState={workspaceState}
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
          <NodeLibrary
            summaries={capabilitySnapshot.summaries}
            loadedSpecs={nodeSpecsFromSnapshot(capabilitySnapshot)}
            onSearch={searchCapabilities}
            onAdd={(reference) => addNode(reference)}
          />
        ) : (
          <AssetLibrary
            assets={assets}
            error={assetError}
            selectedAssetId={selectedAssetId}
            onSelectAsset={setSelectedAssetId}
            onAddToCanvas={(asset) => addAssetSource(asset.id)}
            onJumpToNode={() => setTab("nodes")}
          />
        )}

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
            <InspectorPanel
              node={selected}
              modeOptions={modeOptions}
              onModeChange={changeMode}
              onParamChange={setParam}
            />
          </>
        {assistantEnabled && assistantOpen && (
          <AssistantDock
            key={project?.id ?? "no-project"}
            onClose={() => setAssistantOpen(false)}
            getContext={assistantContext}
            onWorkflowHead={adoptWorkflowHead}
            beforeSend={(restoreFocus) =>
              runAfterBarrier("assistant_turn", () => undefined, restoreFocus)
            }
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
      {closeError !== null && (
        <CloseFailureDialog
          error={closeError}
          onKeepEditing={keepEditing}
          onDiscardAndClose={discardAndClose}
        />
      )}
    </div>
  );
}

function resolveRequestedRef(requested: string | CapabilityRef, snapshot: CapabilityCacheSnapshot): CapabilityRef {
  if (typeof requested !== "string") return requested;
  const summary = snapshot.summaries.find((candidate) => candidate.reference.id === requested);
  if (summary) return summary.reference;
  const loaded = [...snapshot.bundles.values()].find((bundle) => bundle.reference.id === requested);
  return loaded?.reference ?? { id: requested, version: "1.0" };
}

function parseCapabilityRef(encoded: string): string | CapabilityRef {
  try {
    const value: unknown = JSON.parse(encoded);
    if (
      typeof value === "object" &&
      value !== null &&
      "id" in value &&
      "version" in value &&
      typeof value.id === "string" &&
      typeof value.version === "string"
    ) {
      return { id: value.id, version: value.version };
    }
  } catch {
    // Older drag payloads contained only the type id.
  }
  return encoded;
}
