import { ArrowLeft, Copy, Grid2X2, Home, Maximize2, PanelLeft, Rows3, Save, Settings } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";
import type { Asset, CanvasNodeData, GenerationHistory, LibNode, NodeKind, Project, TaskRecord } from "../types";
import {
  AddNodePanel,
  AssetsPanel,
  BottomToolbar,
  CanvasDrawer,
  HelpPanel,
  HistoryPanel,
  ShortcutsModal,
  ToolboxPanel,
  panelTitle,
  type PanelId
} from "./CanvasPanels";
import { Button, IconButton } from "./ui";

export function CanvasTopbar({
  project,
  readonlyProject,
  tasks,
  onNavigateHome,
  onNavigateProjects,
  onRenameProject,
  onCreateEditableCopy,
  onOpenConfig
}: {
  project: Project;
  readonlyProject: boolean;
  tasks: TaskRecord[];
  onNavigateHome: () => void;
  onNavigateProjects: () => void;
  onRenameProject: (name: string) => void;
  onCreateEditableCopy: () => void;
  onOpenConfig: () => void;
}) {
  return (
    <header className="canvas-topbar">
      <div className="canvas-nav">
        <IconButton label="返回首页" onClick={onNavigateHome}>
          <Home size={18} />
        </IconButton>
        <IconButton label="返回项目" onClick={onNavigateProjects}>
          <ArrowLeft size={18} />
        </IconButton>
        <input
          className="project-name-input"
          aria-label="项目名称"
          name="projectName"
          value={project.name}
          readOnly={project.readonly}
          onChange={(event) => onRenameProject(event.target.value)}
        />
        {project.readonly && <span className="pill">只读预览</span>}
        {project.readonly && (
          <Button className="canvas-copy-btn" size="sm" onClick={onCreateEditableCopy}>
            <Copy size={14} />
            创建副本
          </Button>
        )}
      </div>
      <div className="canvas-top-actions">
        <div className="task-pill-group">
          <span className="task-pill">
            本地工作区 · {tasks.filter((task) => task.status !== "done").length} 进行中
          </span>
        </div>
        {!readonlyProject && (
          <IconButton label="保存状态">
            <Save size={18} />
          </IconButton>
        )}
        <IconButton label="系统配置" onClick={onOpenConfig}>
          <Settings size={18} />
        </IconButton>
      </div>
    </header>
  );
}

export function CanvasLeftControls({
  readonlyProject,
  minimap,
  snapToGrid,
  zoom,
  onOrganize,
  onToggleMinimap,
  onToggleSnap,
  onFitView
}: {
  readonlyProject: boolean;
  minimap: boolean;
  snapToGrid: boolean;
  zoom: number;
  onOrganize: () => void;
  onToggleMinimap: () => void;
  onToggleSnap: () => void;
  onFitView: () => void;
}) {
  return (
    <div className="canvas-left-controls" aria-label="画布控制">
      {!readonlyProject && (
        <IconButton label="整理画布" onClick={onOrganize}>
          <Rows3 size={16} />
        </IconButton>
      )}
      <IconButton label="切换小地图" className={minimap ? "active" : ""} onClick={onToggleMinimap}>
        <PanelLeft size={16} />
      </IconButton>
      {!readonlyProject && (
        <IconButton label="网格吸附" className={snapToGrid ? "active" : ""} onClick={onToggleSnap}>
          <Grid2X2 size={16} />
        </IconButton>
      )}
      <span className="zoom-pill">{Math.round(zoom * 100)}%</span>
      <IconButton label="适配视图" onClick={onFitView}>
        <Maximize2 size={16} />
      </IconButton>
    </div>
  );
}

export function CanvasPanelHost({
  readonlyProject,
  activePanel,
  setActivePanel,
  onAddNode,
  onUpload,
  history,
  onImportHistory,
  onUseToolboxPreset,
  assets,
  tasks,
  onImportAsset,
  setHistory
}: {
  readonlyProject: boolean;
  activePanel: PanelId;
  setActivePanel: Dispatch<SetStateAction<PanelId>>;
  onAddNode: (kind: NodeKind, name: string, extra?: Partial<CanvasNodeData>) => LibNode | undefined;
  onUpload: (files: FileList | File[]) => void;
  history: GenerationHistory[];
  onImportHistory: (item: GenerationHistory) => void;
  onUseToolboxPreset: (presetId: string) => void;
  assets: Asset[];
  tasks: TaskRecord[];
  onImportAsset: (asset: Asset) => void;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
}) {
  if (readonlyProject) {
    return null;
  }

  return (
    <>
      <BottomToolbar activePanel={activePanel} setActivePanel={setActivePanel} />
      {activePanel && activePanel !== "shortcuts" && (
        <CanvasDrawer panel={activePanel} title={panelTitle(activePanel)} onClose={() => setActivePanel(null)}>
          {activePanel === "add" && (
            <AddNodePanel
              onAdd={onAddNode}
              onUpload={onUpload}
              history={history}
              onImportHistory={onImportHistory}
            />
          )}
          {activePanel === "toolbox" && <ToolboxPanel onUse={onUseToolboxPreset} />}
          {activePanel === "assets" && (
            <AssetsPanel assets={assets} onUpload={onUpload} onImport={onImportAsset} />
          )}
          {activePanel === "history" && (
            <HistoryPanel history={history} tasks={tasks} setHistory={setHistory} onImport={onImportHistory} />
          )}
          {activePanel === "help" && <HelpPanel />}
        </CanvasDrawer>
      )}
      {activePanel === "shortcuts" && <ShortcutsModal onClose={() => setActivePanel(null)} />}
    </>
  );
}
