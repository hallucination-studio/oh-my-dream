import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
} from "@xyflow/react";
import { capabilityKey } from "./nodes/exactCapability.ts";
import { useNodeAvailability } from "./nodes/useNodeAvailability.ts";
import type { FlowNodeData } from "./nodes/WorkflowFlowNode.tsx";
import { typeColor } from "./nodes/typeColor.ts";
import { nextNodeId, upsertIncomingEdge } from "./workflow/editor.ts";
import { isValidConnection } from "./workflow/validate.ts";
import {
  api,
  type CapabilityRef,
  type NodeCapabilityContractDto,
  type Project,
  type WorkflowRunDto,
} from "./api/index.ts";
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
import { WorkflowCanvas } from "./canvas/WorkflowCanvas.tsx";
import { useProjectWorkspace } from "./workflow/useProjectWorkspace.ts";
import { useRunController } from "./workflow/useRunController.ts";
import { AssistantDock } from "./assistant/AssistantDock.tsx";
import { useAssistantAvailability } from "./assistant/useAssistantAvailability.ts";
import { CloseFailureDialog } from "./components/CloseFailureDialog.tsx";
import { parseCapabilityRef } from "./workflow/capabilitySelection.ts";
import { useNodePresentation } from "./workflow/useNodePresentation.ts";
import { RunDrawer } from "./components/RunDrawer.tsx";

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
  const [runDetailsOpen, setRunDetailsOpen] = useState(false);
  const [runSnapshot, setRunSnapshot] = useState<WorkflowRunDto | null>(null);
  const {
    assistantEnabled,
    assistantOpen,
    setAssistantOpen,
    refreshAssistantEnabled,
  } = useAssistantAvailability();
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(null);
  const [exactContracts, setExactContracts] = useState<NodeCapabilityContractDto[]>([]);
  const { specs: exactSpecs, hiddenCapabilityKeys } = useNodeAvailability(exactContracts);
  useEffect(() => {
    let active = true;
    void api.nodeCapabilityList().then((contracts) => {
      if (active) setExactContracts(contracts);
    });
    return () => {
      active = false;
    };
  }, []);
  const workflowForRunRef = useRef<import("./api/types.ts").WorkflowDto | null>(null);
  const { assets, error: assetError, importAsset, refresh } = useAssets(project?.id ?? null);
  const { applyProgress, reset: resetRun, settle } = useRunProjection(setNodes, setEdges);
  const { cancel, invalidateRun, run: runCanonicalWorkflow } = useRunController({
    getWorkflow: () => workflowForRunRef.current,
    setStatus,
    resetProjection: resetRun,
    applyProgress,
    settleProjection: settle,
    onSucceeded: () => void refresh(),
    onRunChanged: setRunSnapshot,
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
  });
  const savedCapabilityKeys = useMemo(
    () => new Set(
      ("workflowHead" in workspaceState ? workspaceState.workflowHead?.nodes ?? [] : []).map(
        (node) => capabilityKey({ id: node.capability_id, version: node.capability_version }),
      ),
    ),
    [workspaceState],
  );
  workflowForRunRef.current =
    workspaceState.state === "ready" ? workspaceState.workflowHead : null;
  useNodePresentation(workflowForRunRef.current, selectedId, setNodes);
  const run = useCallback(() => {
    void runAfterBarrier("prepare_run", runCanonicalWorkflow).catch((error: unknown) => {
      setStatus({ state: "failed", reason: String(error) });
    });
  }, [runAfterBarrier, runCanonicalWorkflow]);

  const addNode = useCallback(
    (requested: string | CapabilityRef, position?: { x: number; y: number }, contextualParams?: Record<string, unknown>) => {
      if (!canEdit) {
        return;
      }
      const reference = typeof requested === "string"
        ? { id: requested, version: "1.0" }
        : requested;
      const spec = exactSpecs.find((candidate) =>
        candidate.ref.id === reference.id && candidate.ref.version === reference.version
      );
      if (!spec || spec.status.availability !== "available") return;
        if (spec.contextualCreationRoute !== null && contextualParams === undefined) return;
        const params = contextualParams ?? Object.fromEntries(
          spec.params.flatMap((param) =>
            param.default === undefined ? [] : [[param.name, param.default]],
          ),
        );
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
    },
    [canEdit, exactSpecs, markWorkflowMutation, setNodes, setParam],
  );

  const addAssetSource = useCallback((assetId: string, position?: { x: number; y: number }) => {
    const asset = assets.find((candidate) => candidate.id === assetId);
    if (!asset) {
      setStatus({ state: "failed", reason: "Asset is unavailable in the current workspace" });
      return;
    }
    const contract = exactContracts.find(
      (candidate) => candidate.capability_ref.id === `${asset.kind}.read_asset`,
    );
    if (!contract) {
      setStatus({ state: "failed", reason: `This ${asset.kind} asset cannot be added right now` });
      return;
    }
    addNode(contract.capability_ref, position, { asset_id: asset.id });
    setSelectedAssetId(asset.id);
  }, [addNode, assets, exactContracts]);

  useEffect(() => {
    const assetIndex = new Map(assets.map((asset) => [asset.id, asset]));
    setNodes((current) => current.map((node) => {
      const data = node.data as FlowNodeData;
      if (!data.capability?.ref.id.endsWith(".read_asset")) return node;
      const assetId = typeof data.params.asset_id === "string" ? data.params.asset_id : "";
      const asset = assetIndex.get(assetId);
      const kind = data.capability.ref.id.split(".")[0] ?? "asset";
      return {
        ...node,
        data: {
          ...data,
          assetPresentation: asset
            ? { title: asset.displayName, available: asset.contentState === "available" }
            : { title: `Untitled ${kind}`, available: false },
          runtime: asset
            ? { ...data.runtime, state: data.runtime?.state ?? "idle", preview: { kind: asset.kind, url: asset.previewUrl } }
            : { ...data.runtime, state: data.runtime?.state ?? "idle", preview: { kind: kind as "image" | "video" | "audio", url: null } },
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
    return {
      id: node.id,
      type: data.type,
      params: data.params,
      capability: data.capability,
      assetPresentation: data.assetPresentation,
    };
  }, [nodes, selectedId]);
  const assistantContext = useCallback(
    () => ({
      project_id: project?.id ?? null,
      workflow_present: workspaceState.state === "ready" && workspaceState.workflowHead !== null,
      workflow_revision:
        workspaceState.state === "ready" && workspaceState.workflowHead
          ? Number(workspaceState.workflowHead.revision)
          : null,
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
        onOpenRunDetails={() => setRunDetailsOpen(true)}
        hasRunDetails={runSnapshot !== null}
      />
      <ProjectSwitcher
        current={project}
        open={projectsOpen}
        onClose={() => setProjectsOpen(false)}
        onOpenProject={openProject}
        onProjectRenamed={setProject}
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
            contracts={exactContracts}
            loadedSpecs={exactSpecs}
            hiddenCapabilityKeys={hiddenCapabilityKeys}
            savedCapabilityKeys={savedCapabilityKeys}
            onAdd={(reference) => addNode(reference)}
            onOpenAssets={() => setTab("assets")}
          />
        ) : (
          <AssetLibrary
            assets={assets}
            error={assetError}
            selectedAssetId={selectedAssetId}
            onSelectAsset={setSelectedAssetId}
            onAddToCanvas={(asset) => addAssetSource(asset.id)}
            onJumpToNode={() => setTab("nodes")}
            onImport={(kind) => {
              void importAsset(kind).catch((error: unknown) => {
                setStatus({ state: "failed", reason: String(error) });
              });
            }}
          />
        )}

          <>
            <WorkflowCanvas
                nodes={nodes}
                edges={edges}
                onNodesChange={handleNodesChange}
                onEdgesChange={handleEdgesChange}
                onConnect={onConnect}
                onSelectNode={setSelectedId}
                onDrop={onDrop}
            />
            <InspectorPanel
              node={selected}
              modeOptions={[]}
              onParamChange={setParam}
              onRunThroughNode={(nodeId) =>
                void runAfterBarrier("prepare_run", () => runCanonicalWorkflow(nodeId))
                  .catch((error: unknown) => setStatus({ state: "failed", reason: String(error) }))}
              onOpenAssets={() => {
                const assetId = selected?.params.asset_id;
                if (typeof assetId === "string") setSelectedAssetId(assetId);
                setTab("assets");
              }}
            />
          </>
        {assistantEnabled && assistantOpen && (
          <AssistantDock
            key={project?.id ?? "no-project"}
            onClose={() => setAssistantOpen(false)}
            getContext={assistantContext}
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
      <RunDrawer
        open={runDetailsOpen}
        onClose={() => setRunDetailsOpen(false)}
        projectId={project?.id ?? null}
        run={runSnapshot}
        activeNodeId={status.state === "running" ? status.nodeId : null}
        outputPreview={status.state === "succeeded" ? status.outputs : null}
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
