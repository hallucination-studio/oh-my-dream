import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  type NodeProps,
} from "@xyflow/react";
import { useCallback, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { CanvasLeftControls, CanvasPanelHost, CanvasTopbar } from "../components/CanvasChrome";
import { CanvasNavigator } from "../components/CanvasNavigator";
import { ConfigModal } from "../components/ConfigModal";
import { LibNodeComponent, type LibNodeComponentProps } from "../components/LibNode";
import { Modal } from "../components/ui";
import { useCanvasActions } from "../hooks/useCanvasActions";
import { useCanvasWorkspaceState } from "../hooks/useCanvasWorkspaceState";
import { useStore } from "../storage";
import type { Asset, GenerationHistory, LibNode, PreviewResource, Project } from "../types";
import { downloadUrl } from "../utils";

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
    setHistory,
    setTasks,
    batches,
    setBatches
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
    setTasks,
    setBatches,
    setEdges,
    addCanvasNode,
    addNodeNear,
    updateNodeData
  });
  const [preview, setPreview] = useState<PreviewResource | null>(null);

  const downloadHistory = useCallback((item: GenerationHistory) => {
    const resources = item.resultResources ?? [];
    if (resources.length > 0) {
      resources.forEach((resource, index) => {
        const url = resource.dataUrl ?? resource.remoteUrl;
        if (url) {
          downloadUrl(url, `${item.model}-${index + 1}.${resource.mimeType?.split("/")[1] ?? "png"}`);
        }
      });
      return;
    }
    if (item.resultUrl) {
      downloadUrl(item.resultUrl, `${item.model}.png`);
    }
  }, []);

  const downloadAsset = useCallback((asset: Asset) => {
    downloadUrl(asset.url, asset.resource.localPath ?? `${asset.name}.png`);
  }, []);

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
    | "onPreview"
    | "onDownload"
  > | null>(null);
  nodeHandlersRef.current = {
    onUpdate: updateNodeData,
    onOpenAIText: runOpenAIText,
    onOpenAIImage: runOpenAIImage,
    onSeedance: runSeedanceMock,
    onImageTool: runImageTool,
    onDirectorShot: runDirectorShot,
    onStoryboard: generateStoryboard,
    onQuickAction: quickAction,
    onPreview: (resource) => setPreview(resource),
    onDownload: (nodeId) => {
      const node = nodes.find((item) => item.id === nodeId);
      const downloads = node?.data.output?.downloads;
      const resources = node?.data.output?.resources ?? [];
      if (downloads?.length) {
        downloads.forEach((artifact, index) => {
          const resource = resources.find((item) => item.id === artifact.resourceId) ?? resources[index];
          const url = resource?.dataUrl ?? resource?.remoteUrl;
          if (url) {
            downloadUrl(url, artifact.fileName);
          }
        });
      } else if (node?.data.url) {
        downloadUrl(node.data.url, `${node.data.name}.png`);
      }
    }
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
        batches={batches}
        onToggle={() => setNavigatorCollapsed((value) => !value)}
        onLocateNode={locateNode}
        onImportAsset={importAsset}
        onPreviewAsset={setPreview}
        onDownloadAsset={downloadAsset}
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
        onPreview={setPreview}
        onDownloadAsset={downloadAsset}
        onDownloadHistory={downloadHistory}
      />
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
      {preview && <PreviewModal preview={preview} onClose={() => setPreview(null)} />}
    </div>
  );
}

function PreviewModal({ preview, onClose }: { preview: PreviewResource; onClose: () => void }) {
  const active = preview.items[preview.activeIndex ?? 0] ?? preview.items[0];
  const url = active?.dataUrl ?? active?.remoteUrl;
  return (
    <Modal title={preview.title} width={920} onClose={onClose}>
      <div className="preview-modal">
        {preview.kind === "video" ? (
          <video src={url} className="preview-media" controls autoPlay muted playsInline />
        ) : preview.kind === "audio" ? (
          <audio src={url} className="preview-audio" controls autoPlay />
        ) : (
          <img src={url} className="preview-media" alt={preview.title} />
        )}
        <div className="preview-strip">
          {preview.items.map((item) => (
            <article key={item.id}>
              <span>{item.title}</span>
              <small>{item.localPath ?? item.remoteUrl ?? "本地缓存"}</small>
            </article>
          ))}
        </div>
      </div>
    </Modal>
  );
}
