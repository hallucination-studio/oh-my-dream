import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  type NodeProps,
} from "@xyflow/react";
import { useCallback, useMemo, useRef } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { CanvasLeftControls, CanvasPanelHost, CanvasTopbar } from "../components/CanvasChrome";
import { CanvasNavigator } from "../components/CanvasNavigator";
import { ConfigModal } from "../components/ConfigModal";
import { LibNodeComponent, type LibNodeComponentProps } from "../components/LibNode";
import { useCanvasActions } from "../hooks/useCanvasActions";
import { useCanvasWorkspaceState } from "../hooks/useCanvasWorkspaceState";
import { useStore } from "../storage";
import type { LibNode, Project } from "../types";

export function CanvasPage() {
  const { projectId } = useParams();
  const { projects } = useStore();
  const project = projects.find((item) => item.id === projectId);

  if (!project) {
    return (
      <AppShell>
        <main className="not-found">
          <h1>项目不存在</h1>
          <Link to="/project" className="text-link">
            返回项目
          </Link>
        </main>
      </AppShell>
    );
  }

  return (
    <ReactFlowProvider>
      <CanvasWorkspace key={project.id} project={project} />
    </ReactFlowProvider>
  );
}

function CanvasWorkspace({ project }: { project: Project }) {
  const {
    updateProject,
    duplicateProject,
    config,
    ui,
    setUi,
    assets,
    setAssets,
    history,
    setHistory
  } = useStore();
  const navigate = useNavigate();
  const {
    nodes,
    edges,
    setEdges,
    activePanel,
    setActivePanel,
    configOpen,
    setConfigOpen,
    selectedId,
    navigatorCollapsed,
    setNavigatorCollapsed,
    zoom,
    readonlyProject,
    updateNodeData,
    addCanvasNode,
    addNodeNear,
    onNodesChange,
    onEdgesChange,
    onConnect,
    onSelectionChange,
    locateNode,
    persistViewport,
    organizeCanvas,
    fitView
  } = useCanvasWorkspaceState({ project, updateProject });

  const {
    importAsset,
    importHistory,
    handleUpload,
    runOpenAIText,
    runOpenAIImage,
    runSeedanceMock,
    runImageTool,
    runDirectorShot,
    generateStoryboard,
    quickAction,
    insertToolboxPreset
  } = useCanvasActions({
    nodes,
    config,
    setAssets,
    setHistory,
    setEdges,
    addCanvasNode,
    addNodeNear,
    updateNodeData
  });

  const nodeHandlersRef = useRef<Pick<
    LibNodeComponentProps,
    | "onUpdate"
    | "onOpenAIText"
    | "onOpenAIImage"
    | "onSeedance"
    | "onImageTool"
    | "onDirectorShot"
    | "onStoryboard"
    | "onQuickAction"
  > | null>(null);
  nodeHandlersRef.current = {
    onUpdate: updateNodeData,
    onOpenAIText: runOpenAIText,
    onOpenAIImage: runOpenAIImage,
    onSeedance: runSeedanceMock,
    onImageTool: runImageTool,
    onDirectorShot: runDirectorShot,
    onStoryboard: generateStoryboard,
    onQuickAction: quickAction
  };

  const nodeTypes = useMemo(
    () => ({
      libNode: (props: NodeProps<LibNode>) => {
        const handlers = nodeHandlersRef.current;
        if (!handlers) {
          return null;
        }
        return <LibNodeComponent {...props} {...handlers} />;
      }
    }),
    []
  );

  const createEditableCopy = useCallback(() => {
    const copy = duplicateProject(project.id);
    if (copy) {
      navigate(`/canvas/${copy.id}`);
    }
  }, [duplicateProject, navigate, project.id]);

  return (
    <div className={`canvas-page ${navigatorCollapsed ? "navigator-collapsed" : "with-navigator"}`}>
      <CanvasNavigator
        project={project}
        nodes={nodes}
        assets={assets}
        selectedId={selectedId}
        collapsed={navigatorCollapsed}
        onToggle={() => setNavigatorCollapsed((value) => !value)}
        onLocateNode={locateNode}
      />
      <CanvasTopbar
        project={project}
        readonlyProject={readonlyProject}
        onNavigateHome={() => navigate("/")}
        onNavigateProjects={() => navigate("/project")}
        onRenameProject={(name) => updateProject(project.id, { name })}
        onCreateEditableCopy={createEditableCopy}
        onOpenConfig={() => setConfigOpen(true)}
      />

      <ReactFlow
        className={ui.snapToGrid ? "snap-grid" : ""}
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onSelectionChange={onSelectionChange}
        onMoveEnd={persistViewport}
        defaultViewport={project.viewport}
        minZoom={0.08}
        maxZoom={2}
        fitView={nodes.length === 0}
        nodesDraggable={!readonlyProject}
        nodesConnectable={!readonlyProject}
        nodesFocusable={!readonlyProject}
        edgesFocusable={!readonlyProject}
        elementsSelectable={!readonlyProject}
        edgesReconnectable={!readonlyProject}
        snapToGrid={ui.snapToGrid}
        snapGrid={[20, 20]}
        deleteKeyCode={null}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          color={ui.snapToGrid ? "#006666" : "#b9c1c9"}
          gap={ui.snapToGrid ? 20 : 32}
          size={1}
          variant={BackgroundVariant.Dots}
        />
        <Controls showInteractive={false} position="bottom-left" />
        {ui.minimap && <MiniMap pannable zoomable position="bottom-right" />}
      </ReactFlow>

      <CanvasLeftControls
        readonlyProject={readonlyProject}
        minimap={ui.minimap}
        snapToGrid={ui.snapToGrid}
        zoom={zoom}
        onOrganize={organizeCanvas}
        onToggleMinimap={() => setUi((value) => ({ ...value, minimap: !value.minimap }))}
        onToggleSnap={() => setUi((value) => ({ ...value, snapToGrid: !value.snapToGrid }))}
        onFitView={fitView}
      />

      <CanvasPanelHost
        readonlyProject={readonlyProject}
        activePanel={activePanel}
        setActivePanel={setActivePanel}
        onAddNode={addCanvasNode}
        onUpload={handleUpload}
        history={history}
        onImportHistory={importHistory}
        onUseToolboxPreset={insertToolboxPreset}
        assets={assets}
        onImportAsset={importAsset}
        setHistory={setHistory}
      />
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
    </div>
  );
}
