import {
  AudioLines,
  BookOpen,
  Box,
  Boxes,
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
import type { ReactNode } from "react";
import { imageCovers } from "../fixtures";
import { historyDisplayText } from "../services/generation";
import type { CanvasNodeData, GenerationHistory, LibNode, NodeKind } from "../types";
import { IconButton, Modal } from "./ui";
export { AssetsPanel } from "./CanvasAssetsPanel";
export { HistoryPanel } from "./CanvasHistoryPanel";
export { ToolboxPanel } from "./CanvasToolboxPanel";

export type PanelId = "add" | "toolbox" | "assets" | "history" | "shortcuts" | "help" | null;

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
