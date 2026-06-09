import { Clock3, Hammer, HelpCircle, Keyboard, Plus, X } from "lucide-react";
import type { ReactNode } from "react";
import { IconButton, Modal } from "./ui";
export { AddNodePanel } from "./CanvasAddNodePanel";
export { AssetsPanel } from "./CanvasAssetsPanel";
export { HistoryPanel } from "./CanvasHistoryPanel";
export { ToolboxPanel } from "./CanvasToolboxPanel";

export type PanelId = "add" | "toolbox" | "assets" | "history" | "shortcuts" | "help" | null;

export function panelTitle(panel: Exclude<PanelId, null>) {
  const titles = {
    add: "添加节点",
    toolbox: "工具箱",
    assets: "资产",
    history: "历史记录",
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
