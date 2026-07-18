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
import type { FlowNodeData, ConnectHighlight } from "./nodes/WorkflowFlowNode.tsx";
import { typeColor } from "./nodes/typeColor.ts";
import { nextNodeId, upsertIncomingEdge, isPersistedMutation } from "./workflow/editor.ts";
import { firstFreeSlot } from "./canvas/placement.ts";
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
import { failureCopy } from "./workflow/failureCopy.ts";
import { useWorkflowReadiness } from "./workflow/useWorkflowReadiness.ts";
import { projectReadinessIssues } from "./workflow/readinessCopy.ts";
import { useNotice } from "./components/useNotice.ts";
import { RunDrawer } from "./components/RunDrawer.tsx";

function isGraphMutation(change: { type: string; dragging?: boolean }): boolean {
  return isPersistedMutation(change);
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
  const { notice, notify } = useNotice();
  const [connect, setConnect] = useState<ConnectHighlight | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);
  const {
    assistantEnabled,
    assistantOpen,
    setAssistantOpen,
    refreshAssistantEnabled,
  } = useAssistantAvailability();
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(null);
  const [exactContracts, setExactContracts] = useState<NodeCapabilityContractDto[]>([]);
  const [contractsLoaded, setContractsLoaded] = useState(false);
  const { specs: exactSpecs, hiddenCapabilityKeys } = useNodeAvailability(exactContracts);
  useEffect(() => {
    let active = true;
    void api.nodeCapabilityList().then((contracts) => {
      if (active) {
        setExactContracts(contracts);
        setContractsLoaded(true);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  const workflowForRunRef = useRef<import("./api/types.ts").WorkflowDto | null>(null);
  const { assets, error: assetError, loading: assetsLoading, importAsset, refresh } = useAssets(project?.id ?? null);
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
    saving,
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
  const readiness = useWorkflowReadiness(workflowForRunRef.current);
  const runReady = readiness?.state === "ready";
  const runDisabledReason = readiness === null
    ? "Checking whether the workflow is ready to run"
    : readiness.state === "blocked"
      ? "Fix the listed issues before running"
      : null;
  const readinessIssueCopy = useMemo(() => {
    const inputType = (nodeId: string, inputKey: string) => {
      const node = nodes.find((candidate) => candidate.id === nodeId);
      const data = node?.data as FlowNodeData | undefined;
      const type = data?.capability?.inputs.find((port) => port.name === inputKey)?.type;
      return type === "string" ? "text" : type ?? null;
    };
    return projectReadinessIssues(
      readiness,
      inputType,
      nodes.map((node) => node.id),
    ).map((issue) => issue.copy);
  }, [nodes, readiness]);
  const run = useCallback(() => {
    if (nodes.length === 0) {
      notify("Add a node from the library before running");
      return;
    }
    void runAfterBarrier("prepare_run", runCanonicalWorkflow)
      .then(() => setRunDetailsOpen(true))
      .catch((error: unknown) => {
        setStatus({ state: "failed", reason: failureCopy("Run workflow", error) });
      });
  }, [nodes.length, notify, runAfterBarrier, runCanonicalWorkflow]);

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
              position: position ?? firstFreeSlot(current),
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
      notify("Add asset failed · it is unavailable in the current workspace");
      return;
    }
    const contract = exactContracts.find(
      (candidate) => candidate.capability_ref.id === `${asset.kind}.read_asset`,
    );
    if (!contract) {
      notify(`Add asset failed · this ${asset.kind} asset cannot be added right now`);
      return;
    }
    addNode(contract.capability_ref, position, { asset_id: asset.id });
    setSelectedAssetId(asset.id);
  }, [addNode, assets, exactContracts, notify]);

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

  const attemptConnection = useCallback(
    (connection: Connection) => {
      if (!isValidConnection(connection, nodes)) {
        notify("Connection rejected · the port types are incompatible");
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
    [markWorkflowMutation, nodes, notify, setEdges],
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      if (!canEdit) {
        return;
      }
      attemptConnection(connection);
    },
    [attemptConnection, canEdit],
  );

  const sourceTypeFor = useCallback(
    (nodeId: string, handle: string) => {
      const node = nodes.find((candidate) => candidate.id === nodeId);
      const data = node?.data as FlowNodeData | undefined;
      return data?.capability?.outputs.find((port) => port.name === handle)?.type ?? null;
    },
    [nodes],
  );

  const startKeyboardConnect = useCallback(
    (nodeId: string, handle: string) => {
      const sourceType = sourceTypeFor(nodeId, handle);
      if (!canEdit || sourceType === null) return;
      setConnect({ sourceId: nodeId, sourceHandle: handle, sourceType, keyboard: true });
    },
    [canEdit, sourceTypeFor],
  );

  const completeKeyboardConnect = useCallback(
    (targetId: string, targetHandle: string) => {
      if (!connect) return;
      attemptConnection({
        source: connect.sourceId,
        sourceHandle: connect.sourceHandle,
        target: targetId,
        targetHandle,
      });
      setConnect(null);
    },
    [attemptConnection, connect],
  );

  // Keyboard connection intent: focus compatible targets, arrows cycle, Escape cancels.
  useEffect(() => {
    if (!connect?.keyboard) return;
    const focusables = () =>
      Array.from(document.querySelectorAll<HTMLElement>('[data-connect-target="compatible"]'));
    focusables()[0]?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setConnect(null);
        return;
      }
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const items = focusables();
        if (items.length === 0) return;
        const index = items.indexOf(document.activeElement as HTMLElement);
        const step = event.key === "ArrowDown" ? 1 : -1;
        items[(index + step + items.length) % items.length]?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [connect?.keyboard, connect?.sourceId]);

  const deleteNode = useCallback(
    (nodeId: string) => {
      if (!canEdit) return;
      markWorkflowMutation();
      setNodes((current) => current.filter((node) => node.id !== nodeId));
      setEdges((current) =>
        current.filter((edge) => edge.source !== nodeId && edge.target !== nodeId),
      );
      setSelectedId(null);
    },
    [canEdit, markWorkflowMutation, setNodes, setEdges],
  );

  const deleteEdge = useCallback(
    (edgeId: string) => {
      if (!canEdit) return;
      markWorkflowMutation();
      setEdges((current) => current.filter((edge) => edge.id !== edgeId));
      setSelectedEdgeId(null);
    },
    [canEdit, markWorkflowMutation, setEdges],
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
      for (const change of changes) {
        if (change.type === "select") {
          setSelectedEdgeId((current) =>
            change.selected ? change.id : current === change.id ? null : current,
          );
        }
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
  const runNodeLabel = useCallback(
    (nodeId: string) => {
      const node = nodes.find((candidate) => candidate.id === nodeId);
      const data = node?.data as FlowNodeData | undefined;
      return data?.capability?.label ?? "Step";
    },
    [nodes],
  );
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

  const selectedEdge = useMemo(() => {
    if (!selectedEdgeId) return null;
    const edge = edges.find((candidate) => candidate.id === selectedEdgeId);
    if (!edge) return null;
    return {
      id: edge.id,
      sourceLabel: runNodeLabel(edge.source),
      targetLabel: runNodeLabel(edge.target),
    };
  }, [edges, selectedEdgeId, runNodeLabel]);

  const canvasNodes = useMemo(
    () =>
      nodes.map((node) => ({
        ...node,
        data: {
          ...node.data,
          connect,
          onStartKeyboardConnect: startKeyboardConnect,
          onCompleteKeyboardConnect: completeKeyboardConnect,
        }
      })),
    [nodes, connect, startKeyboardConnect, completeKeyboardConnect],
  );

  const validateConnection = useCallback(
    (connection: { source: string; sourceHandle?: string | null; target: string; targetHandle?: string | null }) =>
      isValidConnection(
        {
          source: connection.source,
          sourceHandle: connection.sourceHandle ?? null,
          target: connection.target,
          targetHandle: connection.targetHandle ?? null,
        },
        nodes,
      ),
    [nodes],
  );

  const onDrop = useCallback(
    (e: React.DragEvent, position: { x: number; y: number }) => {
      e.preventDefault();
      const encoded = e.dataTransfer.getData("application/oh-node");
      if (encoded) {
        addNode(parseCapabilityRef(encoded), position);
        return;
      }
      const assetId = e.dataTransfer.getData("application/oh-asset");
      if (assetId) addAssetSource(assetId, position);
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
        runDisabled={!runReady}
        runDisabledReason={runDisabledReason}
        runNodeLabel={runNodeLabel}
        saving={project ? saving : null}
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
            loading={!contractsLoaded}
            onAdd={(reference) => addNode(reference)}
            onOpenAssets={() => setTab("assets")}
          />
        ) : (
          <AssetLibrary
            assets={assets}
            error={assetError}
            loading={assetsLoading}
            hasProject={project !== null}
            selectedAssetId={selectedAssetId}
            onSelectAsset={setSelectedAssetId}
            onAddToCanvas={(asset) => addAssetSource(asset.id)}
            onJumpToNode={(asset) => {
              if (asset.sourceNodeId) {
                setSelectedId(asset.sourceNodeId);
                setFocusedNodeId(asset.sourceNodeId);
              }
              setTab("nodes");
            }}
            onImport={(kind) => {
              void importAsset(kind).catch((error: unknown) => {
                notify(failureCopy("Import asset", error));
              });
            }}
          />
        )}

          <>
            <WorkflowCanvas
                nodes={canvasNodes}
                edges={edges}
                onNodesChange={handleNodesChange}
                onEdgesChange={handleEdgesChange}
                onConnect={onConnect}
                onConnectStart={(nodeId, handleId) => {
                  const sourceType = sourceTypeFor(nodeId, handleId);
                  if (sourceType !== null) {
                    setConnect({ sourceId: nodeId, sourceHandle: handleId, sourceType, keyboard: false });
                  }
                }}
                onConnectEnd={() => setConnect(null)}
                isValidConnection={validateConnection}
                onSelectNode={(nodeId) => {
                  setSelectedId(nodeId);
                  setSelectedEdgeId(null);
                }}
                onDrop={onDrop}
                focusNodeId={focusedNodeId}
            >
              {nodes.length === 0 && (
                <div className="bench__canvas-empty">
                  <b>{project ? "Start your workflow" : "Welcome"}</b>
                  <p>
                    {project
                      ? "Add a Text node from the library to begin."
                      : "Create or open a Project from the top bar to start building."}
                  </p>
                </div>
              )}
            </WorkflowCanvas>
            <InspectorPanel
              node={selected}
              onParamChange={setParam}
              onRunThroughNode={(nodeId) =>
                void runAfterBarrier("prepare_run", () => runCanonicalWorkflow(nodeId))
                  .then(() => setRunDetailsOpen(true))
                  .catch((error: unknown) => setStatus({ state: "failed", reason: failureCopy("Run workflow", error) }))}
              onOpenAssets={() => {
                const assetId = selected?.params.asset_id;
                if (typeof assetId === "string") setSelectedAssetId(assetId);
                setTab("assets");
              }}
              readinessIssues={readinessIssueCopy}
              runDisabled={!runReady}
              onDeleteNode={deleteNode}
              selectedEdge={selectedEdge}
              onDeleteEdge={deleteEdge}
            />
          </>
        {assistantEnabled && (
          <div className={`adock-host${assistantOpen ? "" : " is-hidden"}`} hidden={!assistantOpen}>
            <AssistantDock
              key={project?.id ?? "no-project"}
              onClose={() => setAssistantOpen(false)}
              getContext={assistantContext}
              nodeLabel={runNodeLabel}
              beforeSend={(restoreFocus) =>
                runAfterBarrier("assistant_turn", () => undefined, restoreFocus)
              }
            />
          </div>
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
        nodeLabel={runNodeLabel}
        canCancel={status.state === "running"}
        onCancel={cancel}
      />
      {notice !== null && (
        <div className="bench__notice" role="status">
          {notice}
        </div>
      )}
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
