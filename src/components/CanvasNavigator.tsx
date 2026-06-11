import {
  Boxes,
  ChevronLeft,
  ChevronRight,
  ChevronsUpDown,
  Filter,
  FolderSearch,
  Image as ImageIcon,
  Layers3,
  LocateFixed,
  MoreHorizontal,
  Search
} from "lucide-react";
import { useMemo, useState } from "react";
import { nodeLabels } from "../constants";
import type { Asset, AssetKind, DerivedBatch, LibNode, NodeKind, PreviewResource, Project, TaskRecord } from "../types";
import { Button, IconButton } from "./ui";

const filters: { value: "all" | NodeKind; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "text", label: "文本" },
  { value: "image", label: "图片" },
  { value: "video", label: "视频" },
  { value: "audio", label: "音频" },
  { value: "script", label: "脚本" },
  { value: "director", label: "导演台" },
  { value: "compose", label: "合成" },
  { value: "group", label: "分组" }
];

export function CanvasNavigator({
  project,
  nodes,
  assets,
  selectedId,
  collapsed,
  batches,
  tasks,
  onToggle,
  onLocateNode,
  onImportAsset,
  onPreviewAsset,
  onDownloadAsset
}: {
  project: Project;
  nodes: LibNode[];
  assets: Asset[];
  selectedId: string | null;
  collapsed: boolean;
  batches: DerivedBatch[];
  tasks: TaskRecord[];
  onToggle: () => void;
  onLocateNode: (node: LibNode) => void;
  onImportAsset: (asset: Asset) => void;
  onPreviewAsset: (preview: PreviewResource) => void;
  onDownloadAsset: (asset: Asset) => void;
}) {
  const [tab, setTab] = useState<"canvas" | "assets" | "tasks" | "asset-manager">("canvas");
  const [filter, setFilter] = useState<"all" | NodeKind>("all");
  const [query, setQuery] = useState("");
  const [groupsCollapsed, setGroupsCollapsed] = useState(false);
  const filteredNodes = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return nodes
      .filter((node) => filter === "all" || node.data.kind === filter)
      .filter((node) => {
        if (!keyword) {
          return true;
        }
        return (
          node.data.name.toLowerCase().includes(keyword) ||
          node.data.kind.toLowerCase().includes(keyword) ||
          (node.data.prompt ?? "").toLowerCase().includes(keyword)
        );
      })
      .sort((a, b) => a.data.name.localeCompare(b.data.name, "zh-CN"));
  }, [filter, nodes, query]);
  const groupedNodes = useMemo(
    () => ({
      groups: filteredNodes.filter((node) => node.data.kind === "group"),
      nodes: filteredNodes.filter((node) => node.data.kind !== "group")
    }),
    [filteredNodes]
  );
  const visibleAssets = useMemo(() => assets.slice(0, 36), [assets]);
  const recentAssets = useMemo(() => assets.slice(0, 12), [assets]);
  const visibleTasks = useMemo(() => tasks.slice(0, 18), [tasks]);

  if (collapsed) {
    return (
      <div className="canvas-navigator-collapsed">
        <IconButton label="展开节点侧栏" onClick={onToggle}>
          <ChevronRight size={17} />
        </IconButton>
      </div>
    );
  }

  return (
    <aside className="canvas-navigator" aria-label="画布节点侧栏">
      <header className="canvas-navigator-head">
        <span className="navigator-workspace-label">Workspace</span>
        <IconButton label="收起节点侧栏" onClick={onToggle}>
          <ChevronLeft size={17} />
        </IconButton>
      </header>
      <div className="navigator-title-row">
        <input name="navigatorProjectName" value={project.name} readOnly aria-label="当前画布名称" />
        <small title={project.workspacePath ?? `workspace/${project.id}`}>
          {project.workspacePath ?? `workspace/${project.id}`}
        </small>
      </div>
      <div className="navigator-tabs" role="tablist" aria-label="画布侧栏">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "canvas"}
          className={tab === "canvas" ? "active" : ""}
          onClick={() => setTab("canvas")}
        >
          画布
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "assets"}
          className={tab === "assets" ? "active" : ""}
          onClick={() => setTab("assets")}
        >
          资产
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "asset-manager"}
          className={tab === "asset-manager" ? "active" : ""}
          onClick={() => setTab("asset-manager")}
        >
          资产管理
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "tasks"}
          className={tab === "tasks" ? "active" : ""}
          onClick={() => setTab("tasks")}
        >
          任务
        </button>
      </div>
      {tab === "canvas" ? (
        <div role="tabpanel" aria-label="画布元素">
          <div className="navigator-tools">
            <span>画布元素</span>
            <button type="button" className="navigator-collapse-btn" onClick={() => setGroupsCollapsed((value) => !value)}>
              <ChevronsUpDown size={14} />
              <span>{groupsCollapsed ? "展开全部分组" : "收起全部分组"}</span>
            </button>
            <label>
              <Filter size={14} />
              <select
                aria-label="筛选画布元素"
                name="navigatorNodeFilter"
                value={filter}
                onChange={(event) => setFilter(event.target.value as "all" | NodeKind)}
              >
                {filters.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="navigator-search">
              <Search size={15} />
              <input
                aria-label="搜索画布节点"
                name="navigatorNodeSearch"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索节点"
              />
            </label>
          </div>
          <div className="navigator-list">
            {groupedNodes.groups.length > 0 && !groupsCollapsed && (
              <section className="navigator-group-section">
                {groupedNodes.groups.map((node) => (
                  <article key={node.id} className={`navigator-row navigator-group-row ${selectedId === node.id ? "active" : ""}`}>
                    <button type="button" className="navigator-row-main" onClick={() => onLocateNode(node)}>
                      <NodeThumb node={node} />
                      <span>
                        <strong>{node.data.name}</strong>
                        <small>{nodeLabels[node.data.kind]}</small>
                      </span>
                    </button>
                    <div className="navigator-row-actions">
                      <button type="button" aria-label={`定位到分组 ${node.data.name}`} onClick={() => onLocateNode(node)}>
                        <LocateFixed size={14} />
                      </button>
                      <button type="button" aria-label={`更多操作 ${node.data.name}`}>
                        <MoreHorizontal size={14} />
                      </button>
                    </div>
                  </article>
                ))}
              </section>
            )}
            {groupedNodes.nodes.map((node) => (
              <article key={node.id} className={`navigator-row ${selectedId === node.id ? "active" : ""}`}>
                <button type="button" className="navigator-row-main" onClick={() => onLocateNode(node)}>
                  <NodeThumb node={node} />
                  <span>
                    <strong>{node.data.name}</strong>
                    <small>{nodeLabels[node.data.kind]}</small>
                  </span>
                </button>
                <div className="navigator-row-actions">
                  <button type="button" aria-label={`定位到节点 ${node.data.name}`} onClick={() => onLocateNode(node)}>
                    <LocateFixed size={14} />
                  </button>
                  <button type="button" aria-label={`更多操作 ${node.data.name}`}>
                    <MoreHorizontal size={14} />
                  </button>
                </div>
              </article>
            ))}
          </div>
          <footer className="navigator-footer">
            <Layers3 size={15} />
            <span>共 {nodes.length} 节点</span>
          </footer>
        </div>
      ) : tab === "assets" ? (
        <div role="tabpanel" aria-label="资产管理">
          <div className="navigator-tools">
            <span>资产</span>
          </div>
          <div className="navigator-asset-grid">
            {visibleAssets.length === 0 ? (
              <p>暂无资产</p>
            ) : (
              visibleAssets.map((asset) => (
                <article key={asset.id} className="navigator-asset-card">
                  <AssetThumb kind={asset.kind} url={asset.url} />
                  <strong>{asset.name}</strong>
                  <div className="navigator-asset-actions">
                    <Button
                      size="sm"
                      onClick={() =>
                        onPreviewAsset({
                          id: `navigator-preview-${asset.id}`,
                          title: asset.name,
                          kind: asset.kind,
                          items: [asset.resource]
                        })
                      }
                    >
                      预览
                    </Button>
                    <Button size="sm" onClick={() => onImportAsset(asset)}>
                      插入
                    </Button>
                  </div>
                </article>
              ))
            )}
          </div>
          <footer className="navigator-footer">
            <Boxes size={15} />
            <span>共 {assets.length} 资产</span>
          </footer>
        </div>
      ) : tab === "asset-manager" ? (
        <div role="tabpanel" aria-label="资产管理详情">
          <div className="navigator-tools">
            <span>资产管理</span>
          </div>
          <div className="asset-manager-list">
            {recentAssets.map((asset) => (
              <article key={asset.id} className="asset-manager-card">
                <AssetThumb kind={asset.kind} url={asset.url} />
                <div>
                  <strong>{asset.name}</strong>
                  <small>{asset.resource.localPath ?? "本地缓存"}</small>
                  <span>{asset.tags?.join(" · ") || "项目资产"}</span>
                  <div className="navigator-asset-actions">
                    <Button
                      size="sm"
                      onClick={() =>
                        onPreviewAsset({
                          id: `manager-preview-${asset.id}`,
                          title: asset.name,
                          kind: asset.kind,
                          items: [asset.resource]
                        })
                      }
                    >
                      预览
                    </Button>
                    <Button size="sm" onClick={() => onImportAsset(asset)}>
                      插入
                    </Button>
                    <Button size="sm" onClick={() => onDownloadAsset(asset)}>
                      下载
                    </Button>
                  </div>
                </div>
              </article>
            ))}
          </div>
          <footer className="navigator-footer navigator-footer-wide">
            <FolderSearch size={15} />
            <span>{assets.length} 资产 · {batches.length} 批次</span>
          </footer>
        </div>
      ) : (
        <div role="tabpanel" aria-label="任务队列">
          <div className="navigator-tools">
            <span>任务队列</span>
          </div>
          <div className="navigator-task-list">
            {visibleTasks.length === 0 ? (
              <p>暂无任务记录</p>
            ) : (
              visibleTasks.map((task) => (
                <article key={task.id} className={`navigator-task-card ${task.status}`}>
                  <div>
                    <strong>{task.title}</strong>
                    <span>{task.provider} · {task.status}</span>
                  </div>
                  <progress value={task.progress} max={100} />
                  <small>{task.detail ?? "任务状态会随生成流程自动更新。"}</small>
                </article>
              ))
            )}
          </div>
          <footer className="navigator-footer navigator-footer-wide">
            <FolderSearch size={15} />
            <span>{tasks.length} 任务 · {tasks.filter((task) => task.status === "failed").length} 失败</span>
          </footer>
        </div>
      )}
    </aside>
  );
}

function NodeThumb({ node }: { node: LibNode }) {
  if (node.data.kind === "image" && node.data.url) {
    return <img src={node.data.url} alt="" loading="lazy" />;
  }
  if (node.data.kind === "video" && node.data.url) {
    return <video src={node.data.url} muted preload="metadata" />;
  }
  return (
    <span className={`navigator-node-icon node-${node.data.kind}`}>
      {nodeLabels[node.data.kind].slice(0, 1)}
    </span>
  );
}

function AssetThumb({ kind, url }: { kind: AssetKind; url: string }) {
  if (kind === "image") {
    return <img src={url} alt="" loading="lazy" />;
  }
  if (kind === "video") {
    return <video src={url} muted preload="metadata" />;
  }
  return (
    <div className="navigator-asset-fallback">
      <ImageIcon size={18} />
    </div>
  );
}
