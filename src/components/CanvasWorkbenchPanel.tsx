import {
  Activity,
  Boxes,
  CheckCircle2,
  Clock3,
  Download,
  FileWarning,
  GitBranch,
  Image as ImageIcon,
  Layers3,
  LocateFixed,
  Play,
  RotateCw,
  Sparkles,
  Wand2
} from "lucide-react";
import { useMemo, useState } from "react";
import { formatGenerationParams, historyDisplayText } from "../services/generation";
import type {
  Asset,
  CanvasNodeData,
  DerivedBatch,
  GenerationHistory,
  LibEdge,
  LibNode,
  PreviewResource,
  TaskRecord
} from "../types";
import { formatDate } from "../utils";
import { MediaThumb } from "./CanvasMediaThumb";
import { Button, IconButton } from "./ui";

type WorkbenchTab = "inspector" | "queue" | "review";

const imagePrimaryTools = ["全景 NEW", "多角度", "打光", "九宫格", "高清", "宫格切分"] as const;

export function CanvasWorkbenchPanel({
  selectedNodeId,
  nodes,
  edges,
  tasks,
  history,
  assets,
  batches,
  readonlyProject,
  activeTab,
  onActiveTabChange,
  onUpdateNode,
  onGenerateText,
  onGenerateImage,
  onGenerateVideo,
  onImageTool,
  onDirectorShot,
  onStoryboard,
  onPreview,
  onDownloadNode,
  onDownloadAsset,
  onDownloadHistory,
  onImportHistory,
  onLocateNode
}: {
  selectedNodeId: string | null;
  nodes: LibNode[];
  edges: LibEdge[];
  tasks: TaskRecord[];
  history: GenerationHistory[];
  assets: Asset[];
  batches: DerivedBatch[];
  readonlyProject: boolean;
  activeTab: WorkbenchTab;
  onActiveTabChange: (tab: WorkbenchTab) => void;
  onUpdateNode: (id: string, patch: Partial<CanvasNodeData>) => void;
  onGenerateText: (id: string) => void;
  onGenerateImage: (id: string) => void;
  onGenerateVideo: (id: string, kind?: "video" | "audio" | "compose") => void;
  onImageTool: (id: string, label: string) => void;
  onDirectorShot: (id: string) => void;
  onStoryboard: (id: string) => void;
  onPreview: (preview: PreviewResource) => void;
  onDownloadNode: (id: string) => void;
  onDownloadAsset: (asset: Asset) => void;
  onDownloadHistory: (item: GenerationHistory) => void;
  onImportHistory: (item: GenerationHistory) => void;
  onLocateNode: (node: LibNode) => void;
}) {
  const selectedNode = useMemo(
    () => nodes.find((node) => node.id === selectedNodeId) ?? null,
    [nodes, selectedNodeId]
  );
  const selectedTasks = useMemo(
    () => tasks.filter((task) => task.sourceNodeId === selectedNodeId),
    [selectedNodeId, tasks]
  );
  const selectedHistory = useMemo(
    () => history.filter((item) => item.sourceNodeId === selectedNodeId || item.prompt === selectedNode?.data.prompt),
    [history, selectedNode?.data.prompt, selectedNodeId]
  );
  const selectedAssets = useMemo(
    () => assets.filter((asset) => asset.sourceNodeId === selectedNodeId || asset.batchId === selectedNode?.data.output?.batchId),
    [assets, selectedNode?.data.output?.batchId, selectedNodeId]
  );

  return (
    <aside className="canvas-workbench-panel" aria-label="画布工作台">
      <div className="workbench-panel-tabs" role="tablist" aria-label="工作台面板">
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "inspector"}
          className={activeTab === "inspector" ? "active" : ""}
          onClick={() => onActiveTabChange("inspector")}
        >
          Inspector
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "queue"}
          className={activeTab === "queue" ? "active" : ""}
          onClick={() => onActiveTabChange("queue")}
        >
          Queue
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "review"}
          className={activeTab === "review" ? "active" : ""}
          onClick={() => onActiveTabChange("review")}
        >
          Review
        </button>
      </div>

      {activeTab === "inspector" && (
        <NodeInspector
          node={selectedNode}
          nodes={nodes}
          edges={edges}
          tasks={selectedTasks}
          history={selectedHistory}
          assets={selectedAssets}
          batches={batches}
          readonlyProject={readonlyProject}
          onUpdateNode={onUpdateNode}
          onGenerateText={onGenerateText}
          onGenerateImage={onGenerateImage}
          onGenerateVideo={onGenerateVideo}
          onImageTool={onImageTool}
          onDirectorShot={onDirectorShot}
          onStoryboard={onStoryboard}
          onPreview={onPreview}
          onDownloadNode={onDownloadNode}
          onLocateNode={onLocateNode}
        />
      )}
      {activeTab === "queue" && (
        <GenerationQueuePanel tasks={tasks} nodes={nodes} onLocateNode={onLocateNode} />
      )}
      {activeTab === "review" && (
        <ReviewPanel
          history={history}
          assets={assets}
          batches={batches}
          nodes={nodes}
          onPreview={onPreview}
          onDownloadAsset={onDownloadAsset}
          onDownloadHistory={onDownloadHistory}
          onImportHistory={onImportHistory}
          onLocateNode={onLocateNode}
        />
      )}
    </aside>
  );
}

function NodeInspector({
  node,
  nodes,
  edges,
  tasks,
  history,
  assets,
  batches,
  readonlyProject,
  onUpdateNode,
  onGenerateText,
  onGenerateImage,
  onGenerateVideo,
  onImageTool,
  onDirectorShot,
  onStoryboard,
  onPreview,
  onDownloadNode,
  onLocateNode
}: {
  node: LibNode | null;
  nodes: LibNode[];
  edges: LibEdge[];
  tasks: TaskRecord[];
  history: GenerationHistory[];
  assets: Asset[];
  batches: DerivedBatch[];
  readonlyProject: boolean;
  onUpdateNode: (id: string, patch: Partial<CanvasNodeData>) => void;
  onGenerateText: (id: string) => void;
  onGenerateImage: (id: string) => void;
  onGenerateVideo: (id: string, kind?: "video" | "audio" | "compose") => void;
  onImageTool: (id: string, label: string) => void;
  onDirectorShot: (id: string) => void;
  onStoryboard: (id: string) => void;
  onPreview: (preview: PreviewResource) => void;
  onDownloadNode: (id: string) => void;
  onLocateNode: (node: LibNode) => void;
}) {
  const [tool, setTool] = useState<string>(imagePrimaryTools[0]);
  if (!node) {
    return (
      <div className="workbench-empty">
        <Layers3 size={22} />
        <strong>选择一个节点</strong>
        <p>节点只保留流程摘要，参数、生成、派生和血缘都在这里处理。</p>
      </div>
    );
  }

  const params = node.data.params ?? {};
  const setParam = (key: string, value: string | number | boolean) =>
    onUpdateNode(node.id, { params: { ...params, [key]: value } });
  const upstreamNodes = edges
    .filter((edge) => edge.target === node.id)
    .map((edge) => nodes.find((item) => item.id === edge.source))
    .filter((item): item is LibNode => Boolean(item));
  const downstreamNodes = edges
    .filter((edge) => edge.source === node.id)
    .map((edge) => nodes.find((item) => item.id === edge.target))
    .filter((item): item is LibNode => Boolean(item));
  const preview = node.data.output?.preview;

  return (
    <div className="workbench-scroll">
      <section className="inspector-section inspector-identity">
        <div>
          <span className="eyebrow">Selected Node</span>
          <input
            name={`inspector-${node.id}-name`}
            aria-label="节点名称"
            value={node.data.name}
            readOnly={readonlyProject || Boolean(node.data.readonly)}
            onChange={(event) => onUpdateNode(node.id, { name: event.target.value })}
          />
        </div>
        <span className={`status-chip ${node.data.taskInfo?.status ?? node.data.workflowType ?? "base"}`}>
          {node.data.taskInfo?.status ?? node.data.workflowType ?? "base"}
        </span>
      </section>

      <section className="inspector-section">
        <h3>Prompt / Content</h3>
        <textarea
          name={`inspector-${node.id}-prompt`}
          aria-label="节点提示词"
          value={node.data.text ?? node.data.prompt ?? ""}
          readOnly={readonlyProject || Boolean(node.data.readonly)}
          onChange={(event) => onUpdateNode(node.id, { prompt: event.target.value, text: node.data.kind === "text" || node.data.kind === "script" ? event.target.value : node.data.text })}
          placeholder="写入脚本、画面描述、镜头动作或生成目标"
        />
      </section>

      <section className="inspector-section">
        <h3>Generation</h3>
        {node.data.kind === "text" || node.data.kind === "script" || node.data.kind === "director" ? (
          <TextInspectorControls node={node} readonly={readonlyProject} onGenerateText={onGenerateText} onDirectorShot={onDirectorShot} onStoryboard={onStoryboard} />
        ) : node.data.kind === "image" ? (
          <ImageInspectorControls
            node={node}
            params={params}
            activeTool={tool}
            onToolChange={setTool}
            setParam={setParam}
            readonly={readonlyProject}
            onGenerateImage={onGenerateImage}
            onImageTool={onImageTool}
          />
        ) : node.data.kind === "video" || node.data.kind === "audio" || node.data.kind === "compose" ? (
          <VideoInspectorControls node={node} params={params} setParam={setParam} readonly={readonlyProject} onGenerateVideo={onGenerateVideo} />
        ) : (
          <p className="panel-note">分组节点用于组织画布，不直接生成。</p>
        )}
      </section>

      <section className="inspector-section">
        <h3>Lineage</h3>
        <div className="lineage-grid">
          <LineageColumn title="Inputs" nodes={upstreamNodes} onLocateNode={onLocateNode} />
          <LineageColumn title="Outputs" nodes={downstreamNodes} onLocateNode={onLocateNode} />
        </div>
        <div className="lineage-stats">
          <span><GitBranch size={13} /> {edges.filter((edge) => edge.source === node.id || edge.target === node.id).length} links</span>
          <span><Clock3 size={13} /> {tasks.length} tasks</span>
          <span><Boxes size={13} /> {assets.length} assets</span>
          <span><Activity size={13} /> {batches.filter((batch) => batch.sourceNodeId === node.id).length} batches</span>
        </div>
      </section>

      {node.data.taskInfo && (
        <section className="inspector-section">
          <h3>Task State</h3>
          <div className={`task-summary-card ${node.data.taskInfo.status}`}>
            <strong>{node.data.taskInfo.message ?? node.data.taskInfo.status}</strong>
            <progress value={node.data.taskInfo.progress ?? 0} max={100} />
          </div>
        </section>
      )}

      {(preview || node.data.url) && (
        <section className="inspector-section">
          <h3>Output</h3>
          <div className="inspector-output-actions">
            <Button
              size="sm"
              onClick={() =>
                onPreview(
                  preview ?? {
                    id: `preview-${node.id}`,
                    title: node.data.name,
                    kind: node.data.kind === "video" ? "video" : node.data.kind === "audio" ? "audio" : "image",
                    items: [
                      {
                        id: `resource-${node.id}`,
                        kind: node.data.kind === "video" ? "video" : node.data.kind === "audio" ? "audio" : "image",
                        title: node.data.name,
                        dataUrl: node.data.url,
                        remoteUrl: node.data.remoteUrl,
                        localPath: node.data.localPath,
                        cachePath: node.data.cachePath,
                        createdAt: new Date().toISOString()
                      }
                    ]
                  }
                )
              }
            >
              <ImageIcon size={14} />
              预览
            </Button>
            <Button size="sm" onClick={() => onDownloadNode(node.id)}>
              <Download size={14} />
              下载
            </Button>
          </div>
        </section>
      )}

      {history.length > 0 && (
        <section className="inspector-section">
          <h3>Recent Results</h3>
          <div className="compact-history-list">
            {history.slice(0, 4).map((item) => (
              <article key={item.id}>
                <strong>{historyDisplayText(item)}</strong>
                <small>{item.provider} · {item.status} · {item.progress}%</small>
              </article>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function TextInspectorControls({
  node,
  readonly,
  onGenerateText,
  onDirectorShot,
  onStoryboard
}: {
  node: LibNode;
  readonly: boolean;
  onGenerateText: (id: string) => void;
  onDirectorShot: (id: string) => void;
  onStoryboard: (id: string) => void;
}) {
  return (
    <div className="inspector-action-grid">
      <Button variant="primary" size="sm" onClick={() => onGenerateText(node.id)} disabled={readonly || Boolean(node.data.readonly)}>
        <Wand2 size={14} />
        生成文本
      </Button>
      {node.data.kind === "director" && (
        <Button size="sm" onClick={() => onDirectorShot(node.id)} disabled={readonly || Boolean(node.data.readonly)}>
          截图为参考
        </Button>
      )}
      {node.data.kind === "script" && (
        <Button size="sm" onClick={() => onStoryboard(node.id)} disabled={readonly || Boolean(node.data.readonly)}>
          生成故事板
        </Button>
      )}
    </div>
  );
}

function ImageInspectorControls({
  node,
  params,
  activeTool,
  onToolChange,
  setParam,
  readonly,
  onGenerateImage,
  onImageTool
}: {
  node: LibNode;
  params: CanvasNodeData["params"];
  activeTool: string;
  onToolChange: (tool: string) => void;
  setParam: (key: string, value: string | number | boolean) => void;
  readonly: boolean;
  onGenerateImage: (id: string) => void;
  onImageTool: (id: string, label: string) => void;
}) {
  return (
    <div className="inspector-control-stack">
      <div className="inspector-grid-two">
        <label>
          <span>能力</span>
          <select value={String(params?.provider ?? "volcengine-ark")} onChange={(event) => setParam("provider", event.target.value)} disabled={readonly || Boolean(node.data.readonly)}>
            <option value="volcengine-ark">火山 Seedream</option>
            <option value="openai">OpenAI 图片</option>
          </select>
        </label>
        <label>
          <span>数量</span>
          <select value={String(params?.count ?? 1)} onChange={(event) => setParam("count", Number(event.target.value))} disabled={readonly || Boolean(node.data.readonly)}>
            <option value={1}>1 张</option>
            <option value={2}>2 张</option>
            <option value={4}>4 张</option>
            <option value={9}>9 张</option>
          </select>
        </label>
        <label>
          <span>模型</span>
          <input value={String(params?.model ?? "Seedream 5.0 Lite")} onChange={(event) => setParam("model", event.target.value)} disabled={readonly || Boolean(node.data.readonly)} />
        </label>
        <label>
          <span>比例</span>
          <input value={String(params?.ratio ?? "16:9 · 2K")} onChange={(event) => setParam("ratio", event.target.value)} disabled={readonly || Boolean(node.data.readonly)} />
        </label>
      </div>
      <label className="inline-check">
        <input type="checkbox" checked={Boolean(params?.grouped ?? false)} onChange={(event) => setParam("grouped", event.target.checked)} disabled={readonly || Boolean(node.data.readonly)} />
        <span>组图模式</span>
      </label>
      <div className="inspector-tool-strip">
        {imagePrimaryTools.map((label) => (
          <button key={label} type="button" className={activeTool === label ? "active" : ""} onClick={() => onToolChange(label)}>
            {label}
          </button>
        ))}
      </div>
      <div className="inspector-action-grid">
        <Button variant="primary" size="sm" onClick={() => onGenerateImage(node.id)} disabled={readonly || Boolean(node.data.readonly)}>
          <Sparkles size={14} />
          生成图片
        </Button>
        <Button size="sm" onClick={() => onImageTool(node.id, activeTool)} disabled={readonly || Boolean(node.data.readonly)}>
          <RotateCw size={14} />
          派生 {activeTool}
        </Button>
      </div>
    </div>
  );
}

function VideoInspectorControls({
  node,
  params,
  setParam,
  readonly,
  onGenerateVideo
}: {
  node: LibNode;
  params: CanvasNodeData["params"];
  setParam: (key: string, value: string | number | boolean) => void;
  readonly: boolean;
  onGenerateVideo: (id: string, kind?: "video" | "audio" | "compose") => void;
}) {
  const disabled = readonly || Boolean(node.data.readonly);
  return (
    <div className="inspector-control-stack">
      <div className="inspector-grid-two">
        <label>
          <span>能力</span>
          <select value={String(params?.provider ?? "volcengine-ark")} onChange={(event) => setParam("provider", event.target.value)} disabled={disabled}>
            <option value="volcengine-ark">火山 Seedance</option>
            <option value="seedance-mock">Seedance Mock</option>
          </select>
        </label>
        <label>
          <span>模式</span>
          <select value={String(params?.modeType ?? "text2video")} onChange={(event) => setParam("modeType", event.target.value)} disabled={disabled}>
            <option value="text2video">text2video</option>
            <option value="image2video">image2video</option>
          </select>
        </label>
        <label>
          <span>比例</span>
          <select value={String(params?.ratio ?? "16:9")} onChange={(event) => setParam("ratio", event.target.value)} disabled={disabled}>
            <option>16:9</option>
            <option>9:16</option>
            <option>1:1</option>
            <option>21:9</option>
          </select>
        </label>
        <label>
          <span>清晰度</span>
          <select value={String(params?.resolution ?? "720p")} onChange={(event) => setParam("resolution", event.target.value)} disabled={disabled}>
            <option>480p</option>
            <option>720p</option>
            <option>1080p</option>
          </select>
        </label>
        <label>
          <span>时长</span>
          <select value={String(params?.duration ?? 5)} onChange={(event) => setParam("duration", Number(event.target.value))} disabled={disabled}>
            <option value={4}>4s</option>
            <option value={5}>5s</option>
            <option value={8}>8s</option>
            <option value={10}>10s</option>
            <option value={15}>15s</option>
          </select>
        </label>
      </div>
      <label className="inline-check">
        <input type="checkbox" checked={Boolean(params?.generateAudio ?? true)} onChange={(event) => setParam("generateAudio", event.target.checked)} disabled={disabled} />
        <span>模型生成声音</span>
      </label>
      <Button variant="primary" size="sm" onClick={() => onGenerateVideo(node.id, node.data.kind === "compose" ? "compose" : node.data.kind === "audio" ? "audio" : "video")} disabled={disabled}>
        <Play size={14} />
        {node.data.kind === "compose" ? "生成合成视频" : node.data.kind === "audio" ? "作为参考输入" : "生成视频"}
      </Button>
    </div>
  );
}

function LineageColumn({ title, nodes, onLocateNode }: { title: string; nodes: LibNode[]; onLocateNode: (node: LibNode) => void }) {
  return (
    <div className="lineage-column">
      <span>{title}</span>
      {nodes.length === 0 ? (
        <small>None</small>
      ) : (
        nodes.slice(0, 5).map((node) => (
          <button key={node.id} type="button" onClick={() => onLocateNode(node)}>
            <LocateFixed size={12} />
            {node.data.name}
          </button>
        ))
      )}
    </div>
  );
}

export function GenerationQueuePanel({
  tasks,
  nodes,
  onLocateNode
}: {
  tasks: TaskRecord[];
  nodes: LibNode[];
  onLocateNode: (node: LibNode) => void;
}) {
  const ordered = [...tasks].sort((a, b) => +new Date(b.updatedAt) - +new Date(a.updatedAt));
  if (ordered.length === 0) {
    return (
      <div className="workbench-empty">
        <CheckCircle2 size={22} />
        <strong>没有生成任务</strong>
        <p>新的生成、派生和下载任务会在这里显示，并保留可定位的来源。</p>
      </div>
    );
  }
  return (
    <div className="workbench-scroll queue-list">
      {ordered.map((task) => {
        const sourceNode = nodes.find((node) => node.id === task.sourceNodeId);
        return (
          <article key={task.id} className={`queue-card ${task.status}`}>
            <header>
              <span className={`status-dot ${task.status}`} />
              <strong>{task.title}</strong>
              <small>{task.status}</small>
            </header>
            <progress value={task.progress} max={100} />
            <p>{task.detail ?? task.provider}</p>
            <footer>
              <span>{task.provider} · {formatDate(task.updatedAt)}</span>
              {sourceNode && (
                <IconButton label={`定位 ${sourceNode.data.name}`} onClick={() => onLocateNode(sourceNode)}>
                  <LocateFixed size={14} />
                </IconButton>
              )}
            </footer>
          </article>
        );
      })}
    </div>
  );
}

export function ReviewPanel({
  history,
  assets,
  batches,
  nodes,
  onPreview,
  onDownloadAsset,
  onDownloadHistory,
  onImportHistory,
  onLocateNode
}: {
  history: GenerationHistory[];
  assets: Asset[];
  batches: DerivedBatch[];
  nodes: LibNode[];
  onPreview: (preview: PreviewResource) => void;
  onDownloadAsset: (asset: Asset) => void;
  onDownloadHistory: (item: GenerationHistory) => void;
  onImportHistory: (item: GenerationHistory) => void;
  onLocateNode: (node: LibNode) => void;
}) {
  const orderedHistory = [...history].sort((a, b) => +new Date(b.createdAt) - +new Date(a.createdAt));
  return (
    <div className="workbench-scroll review-surface">
      <section className="review-summary">
        <article><strong>{history.length}</strong><span>results</span></article>
        <article><strong>{assets.length}</strong><span>assets</span></article>
        <article><strong>{batches.length}</strong><span>batches</span></article>
      </section>
      {orderedHistory.length === 0 ? (
        <div className="workbench-empty inline">
          <FileWarning size={20} />
          <strong>暂无结果</strong>
        </div>
      ) : (
        orderedHistory.slice(0, 24).map((item) => {
          const sourceNode = nodes.find((node) => node.id === item.sourceNodeId);
          return (
            <article key={item.id} className={`review-card ${item.status}`}>
              <MediaThumb kind={item.kind} url={item.resultUrl} />
              <div>
                <header>
                  <strong>{historyDisplayText(item)}</strong>
                  <span>{item.status} · {item.progress}%</span>
                </header>
                <p>{item.params ? formatGenerationParams(item.params) : item.provider}</p>
                <small>{item.model} · {formatDate(item.createdAt)}</small>
                <div className="review-actions">
                  {item.resultResources?.length ? (
                    <Button
                      size="sm"
                      onClick={() =>
                        onPreview({
                          id: `review-preview-${item.id}`,
                          title: historyDisplayText(item),
                          kind: item.kind === "text" ? "image" : item.kind,
                          items: item.resultResources ?? []
                        })
                      }
                    >
                      预览
                    </Button>
                  ) : null}
                  <Button size="sm" onClick={() => onImportHistory(item)}>
                    导入画布
                  </Button>
                  {(item.resultUrl || item.resultResources?.length) && (
                    <Button size="sm" onClick={() => onDownloadHistory(item)}>
                      下载
                    </Button>
                  )}
                  {sourceNode && (
                    <IconButton label={`定位 ${sourceNode.data.name}`} onClick={() => onLocateNode(sourceNode)}>
                      <LocateFixed size={14} />
                    </IconButton>
                  )}
                </div>
              </div>
            </article>
          );
        })
      )}
      {assets.length > 0 && (
        <section className="review-assets">
          <h3>Asset Lineage</h3>
          {assets.slice(0, 10).map((asset) => (
            <article key={asset.id}>
              <MediaThumb kind={asset.kind} url={asset.url} />
              <span>
                <strong>{asset.name}</strong>
                <small>{asset.sourceNodeId ? "generated from node" : asset.batchId ? "derived batch" : "local/imported"}</small>
              </span>
              <Button size="sm" onClick={() => onDownloadAsset(asset)}>
                下载
              </Button>
            </article>
          ))}
        </section>
      )}
    </div>
  );
}
