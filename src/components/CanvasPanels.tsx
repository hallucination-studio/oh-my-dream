import {
  AudioLines,
  BookOpen,
  Box,
  Boxes,
  CircleHelp,
  Clock3,
  FileImage,
  FileText,
  FileVideo,
  Film,
  Grid2X2,
  Hammer,
  HelpCircle,
  Image as ImageIcon,
  Keyboard,
  Mic2,
  Music,
  PenLine,
  Plus,
  SquareSplitHorizontal,
  Type,
  Upload,
  Video,
  X
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { imageCovers, toolboxPresets } from "../fixtures";
import { formatGenerationParams, historyDisplayText } from "../services/generation";
import type { Asset, AssetCategory, CanvasNodeData, GenerationHistory, LibNode, NodeKind } from "../types";
import { formatDate } from "../utils";
import { Button, IconButton, Modal } from "./ui";

export type PanelId = "add" | "toolbox" | "assets" | "history" | "shortcuts" | "help" | null;

const categories: { id: AssetCategory; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "other", label: "其它" },
  { id: "character", label: "人物" },
  { id: "scene", label: "场景" },
  { id: "object", label: "物品" },
  { id: "style", label: "风格" },
  { id: "sound", label: "音效" },
  { id: "project", label: "项目空间" }
];

export function panelTitle(panel: Exclude<PanelId, null>) {
  const titles = {
    add: "添加节点",
    toolbox: "工具箱",
    assets: "我的素材",
    history: "历史资产",
    shortcuts: "快捷键",
    help: "帮助中心"
  };
  return titles[panel];
}

export function BottomToolbar({
  activePanel,
  setActivePanel
}: {
  activePanel: PanelId;
  setActivePanel: (panel: PanelId) => void;
}) {
  const tools: { id: Exclude<PanelId, null>; label: string; icon: ReactNode }[] = [
    { id: "add", label: "添加节点", icon: <Plus size={16} /> },
    { id: "toolbox", label: "工具箱", icon: <Hammer size={16} /> },
    { id: "assets", label: "我的素材", icon: <Boxes size={16} /> },
    { id: "history", label: "历史记录", icon: <Clock3 size={16} /> },
    { id: "shortcuts", label: "快捷键", icon: <Keyboard size={16} /> },
    { id: "help", label: "帮助中心", icon: <HelpCircle size={16} /> }
  ];
  return (
    <div className="bottom-toolbar" aria-label="画布底部工具栏">
      {tools.map((tool) => (
        <IconButton
          key={tool.id}
          label={tool.label}
          className={activePanel === tool.id ? "active" : ""}
          onClick={() => setActivePanel(activePanel === tool.id ? null : tool.id)}
        >
          {tool.icon}
        </IconButton>
      ))}
    </div>
  );
}

export function CanvasDrawer({
  panel,
  title,
  children,
  onClose
}: {
  panel: Exclude<PanelId, null>;
  title: string;
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <aside className={`canvas-drawer drawer-${panel}`} aria-label={title}>
      <header>
        <h2>{title}</h2>
        <IconButton label="关闭面板" onClick={onClose}>
          <X size={16} />
        </IconButton>
      </header>
      {children}
    </aside>
  );
}

export function AddNodePanel({
  onAdd,
  onUpload,
  history,
  onImportHistory
}: {
  onAdd: (kind: NodeKind, name: string, extra?: Partial<CanvasNodeData>) => LibNode | undefined;
  onUpload: (files: FileList | File[]) => void;
  history: GenerationHistory[];
  onImportHistory: (item: GenerationHistory) => void;
}) {
  const groups: { title: string; entries: { kind: NodeKind; name: string; badge?: string; icon: ReactNode }[] }[] = [
    {
      title: "文本",
      entries: [
        { kind: "text", name: "剧本", icon: <FileText size={16} /> },
        { kind: "text", name: "广告词", icon: <Type size={16} /> },
        { kind: "text", name: "品牌文案", icon: <PenLine size={16} /> }
      ]
    },
    {
      title: "图片",
      entries: [
        { kind: "image", name: "海报", icon: <FileImage size={16} /> },
        { kind: "image", name: "分镜", icon: <Grid2X2 size={16} /> },
        { kind: "image", name: "角色设计", icon: <ImageIcon size={16} /> }
      ]
    },
    {
      title: "视频",
      entries: [
        { kind: "video", name: "创意广告", icon: <FileVideo size={16} /> },
        { kind: "video", name: "动画", icon: <Film size={16} /> },
        { kind: "video", name: "电影", icon: <Video size={16} /> }
      ]
    },
    {
      title: "增强",
      entries: [
        { kind: "compose", name: "视频合成", badge: "Beta", icon: <SquareSplitHorizontal size={16} /> },
        { kind: "director", name: "导演台", badge: "NEW", icon: <Box size={16} /> },
        { kind: "script", name: "脚本", badge: "Beta", icon: <BookOpen size={16} /> }
      ]
    },
    {
      title: "音频",
      entries: [
        { kind: "audio", name: "音效", icon: <AudioLines size={16} /> },
        { kind: "audio", name: "配音", icon: <Mic2 size={16} /> },
        { kind: "audio", name: "音乐", icon: <Music size={16} /> }
      ]
    }
  ];
  const latestHistory = history.filter((item) => item.status === "done").slice(0, 4);

  return (
    <div className="drawer-body">
      {groups.map((group) => (
        <section className="node-group" key={group.title}>
          <h3>{group.title}</h3>
          <div className="node-entry-grid">
            {group.entries.map((entry) => (
              <button
                key={`${entry.kind}-${entry.name}`}
                type="button"
                className="node-entry"
                onClick={() =>
                  onAdd(entry.kind, entry.name, {
                    prompt: `${entry.name} 节点`,
                    url: entry.kind === "image" ? imageCovers[Math.floor(Math.random() * imageCovers.length)] : undefined
                  })
                }
              >
                {entry.icon}
                <span>{entry.name}</span>
                {entry.badge && <em>{entry.badge}</em>}
              </button>
            ))}
          </div>
        </section>
      ))}
      <section className="node-group">
        <h3>添加资源</h3>
        <label className="upload-zone">
          <Upload size={18} />
          <span>上传图片、视频、音频文件</span>
          <input
            name="addNodeUpload"
            type="file"
            multiple
            accept="image/*,video/*,audio/*"
            onChange={(event) => event.target.files && onUpload(event.target.files)}
          />
        </label>
        <div className="history-mini-list">
          {latestHistory.length === 0 ? (
            <p className="empty-copy">暂无生成历史可选。</p>
          ) : (
            latestHistory.map((item) => (
              <button key={item.id} type="button" onClick={() => onImportHistory(item)}>
                <Clock3 size={14} />
                <span>{historyDisplayText(item)}</span>
              </button>
            ))
          )}
        </div>
      </section>
    </div>
  );
}

export function ToolboxPanel({ onUse }: { onUse: (presetId: string) => void }) {
  return (
    <div className="drawer-body">
      <div className="toolbox-tabs">
        <button type="button" className="active">我的工具箱</button>
        <Button size="sm">
          <CircleHelp size={14} />
          模板说明
        </Button>
      </div>
      <div className="toolbox-grid">
        {toolboxPresets.map((preset) => (
          <article key={preset.id} className="tool-card">
            <img src={preset.thumb} alt="" loading="lazy" />
            <h3>{preset.name}</h3>
            <p>{preset.description}</p>
            <Button size="sm" variant="primary" onClick={() => onUse(preset.id)}>
              使用
            </Button>
          </article>
        ))}
      </div>
    </div>
  );
}

export function AssetsPanel({
  assets,
  onUpload,
  onImport
}: {
  assets: Asset[];
  onUpload: (files: FileList | File[]) => void;
  onImport: (asset: Asset) => void;
}) {
  const [tab, setTab] = useState<"assets" | "subjects">("assets");
  const [category, setCategory] = useState<AssetCategory>("all");
  const filtered = assets.filter((asset) => category === "all" || asset.category === category);

  return (
    <div className="drawer-body">
      <div className="tab-row compact">
        <button type="button" className={tab === "assets" ? "active" : ""} onClick={() => setTab("assets")}>
          我的素材
        </button>
        <button type="button" className={tab === "subjects" ? "active" : ""} onClick={() => setTab("subjects")}>
          我的主体库
        </button>
      </div>
      <div className="chip-row">
        {categories.map((item) => (
          <button
            key={item.id}
            type="button"
            className={item.id === category ? "active" : ""}
            onClick={() => setCategory(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <label className="upload-zone small">
        <Upload size={16} />
        <span>上传素材</span>
        <input
          name="assetUpload"
          type="file"
          multiple
          accept="image/*,video/*,audio/*"
          onChange={(event) => event.target.files && onUpload(event.target.files)}
        />
      </label>
      {filtered.length === 0 ? (
        <p className="empty-copy">暂无素材。</p>
      ) : (
        <div className="asset-grid">
          {filtered.map((asset) => (
            <article className="asset-card" key={asset.id}>
              <MediaThumb kind={asset.kind} url={asset.url} />
              <strong>{asset.name}</strong>
              {asset.model && (
                <span className="asset-source">
                  {asset.provider ?? "local"} · {asset.model}
                </span>
              )}
              {asset.params && <p className="asset-params">{formatGenerationParams(asset.params)}</p>}
              {asset.prompt && <p className="asset-prompt">{asset.prompt}</p>}
              <Button size="sm" onClick={() => onImport(asset)}>
                插入画布
              </Button>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

export function HistoryPanel({
  history,
  setHistory,
  onImport
}: {
  history: GenerationHistory[];
  setHistory: React.Dispatch<React.SetStateAction<GenerationHistory[]>>;
  onImport: (item: GenerationHistory) => void;
}) {
  const [tab, setTab] = useState<GenerationHistory["kind"]>("text");
  const [size, setSize] = useState(92);
  const [selected, setSelected] = useState<string[]>([]);
  const items = history
    .filter((item) => item.kind === tab)
    .sort((a, b) => +new Date(b.createdAt) - +new Date(a.createdAt));

  const removeSelected = () => {
    setHistory((records) => records.filter((item) => !selected.includes(item.id)));
    setSelected([]);
  };
  const selectTab = (next: GenerationHistory["kind"]) => {
    setTab(next);
    setSelected([]);
  };

  return (
    <div className="drawer-body">
      <div className="history-head">
        <div className="tab-row compact">
          <button type="button" className={tab === "text" ? "active" : ""} onClick={() => selectTab("text")}>
            文本历史
          </button>
          <button type="button" className={tab === "image" ? "active" : ""} onClick={() => selectTab("image")}>
            图片历史
          </button>
          <button type="button" className={tab === "video" ? "active" : ""} onClick={() => selectTab("video")}>
            视频历史
          </button>
          <button type="button" className={tab === "audio" ? "active" : ""} onClick={() => selectTab("audio")}>
            音频历史
          </button>
        </div>
        <label className="range-control">
          <span>缩略图</span>
          <input
            name="historyThumbSize"
            type="range"
            min={68}
            max={142}
            value={size}
            onChange={(event) => setSize(Number(event.target.value))}
          />
        </label>
      </div>
      <div className="batch-row">
        <Button size="sm" onClick={() => setSelected(items.map((item) => item.id))}>
          全选
        </Button>
        <Button size="sm" variant="danger" disabled={selected.length === 0} onClick={removeSelected}>
          批量删除
        </Button>
      </div>
      {items.length === 0 ? (
        <p className="empty-copy">暂无历史记录。</p>
      ) : (
        <div className="history-list">
          {items.map((item) => (
            <article key={item.id} className="history-card" style={{ gridTemplateColumns: `${size}px 1fr` }}>
              <label className="check-cell">
                <input
                  type="checkbox"
                  checked={selected.includes(item.id)}
                  onChange={(event) =>
                    setSelected((values) =>
                      event.target.checked
                        ? [...values, item.id]
                        : values.filter((value) => value !== item.id)
                    )
                  }
                />
                <MediaThumb kind={item.kind} url={item.resultUrl} />
              </label>
              <div>
                <strong>{historyDisplayText(item)}</strong>
                {item.resultText && <p className="history-prompt">Prompt: {item.prompt}</p>}
                {item.revisedPrompt && <p className="history-prompt">修订 Prompt: {item.revisedPrompt}</p>}
                {item.params && <p className="history-prompt">参数: {formatGenerationParams(item.params)}</p>}
                {item.error && <p className="history-error">{item.error}</p>}
                <span>{item.model} · {formatDate(item.createdAt)}</span>
                <em className={`history-status ${item.status}`}>
                  {item.provider} · {item.status} · {item.progress}%
                </em>
                <div className="history-actions">
                  <Button size="sm" onClick={() => onImport(item)}>
                    导入画布
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => setHistory((records) => records.filter((record) => record.id !== item.id))}
                  >
                    删除
                  </Button>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

function MediaThumb({ kind, url }: { kind: GenerationHistory["kind"]; url?: string }) {
  if (kind === "text") {
    return (
      <div className="audio-thumb text-thumb">
        <FileText size={22} />
      </div>
    );
  }
  if (kind === "image" && url) {
    return <img className="media-thumb" src={url} alt="" loading="lazy" />;
  }
  if (kind === "video" && url) {
    return <video className="media-thumb" src={url} muted />;
  }
  if (kind === "audio") {
    return (
      <div className="audio-thumb">
        <AudioLines size={22} />
      </div>
    );
  }
  return (
    <div className="audio-thumb">
      <ImageIcon size={22} />
    </div>
  );
}

export function ShortcutsModal({ onClose }: { onClose: () => void }) {
  const shortcuts = [
    ["Delete", "删除选中节点/边"],
    ["Cmd/Ctrl + C", "复制节点"],
    ["Cmd/Ctrl + V", "粘贴节点"],
    ["Cmd/Ctrl + Z", "撤销"],
    ["Space", "拖拽画布模式"],
    ["Option/Alt + Shift + F", "整理画布"],
    ["Esc", "取消选中/关闭面板"]
  ];
  return (
    <Modal title="快捷键" onClose={onClose} width={520}>
      <div className="shortcut-list">
        {shortcuts.map(([key, value]) => (
          <div key={key}>
            <kbd>{key}</kbd>
            <span>{value}</span>
          </div>
        ))}
      </div>
    </Modal>
  );
}

export function HelpPanel() {
  const items = [
    ["使用教程", "创建项目后通过底部工具栏添加节点、素材、历史或工具箱模板。"],
    ["配置说明", "OpenAI Key 只保存在浏览器本地；Seedance 当前为 mock 生成。"],
    ["本地数据说明", "项目、素材、历史和配置写入 localStorage。"],
    ["关于本地版", "仅保留创作工作流，本地数据不连接商业入口。"]
  ];
  return (
    <div className="drawer-body help-list">
      {items.map(([title, copy]) => (
        <article key={title}>
          <h3>{title}</h3>
          <p>{copy}</p>
        </article>
      ))}
    </div>
  );
}
