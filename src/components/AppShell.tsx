import { Archive, Settings, Sparkles, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { useStore } from "../storage";
import { HelpPanel } from "./CanvasPanels";
import { ConfigModal } from "./ConfigModal";
import { IconButton, Modal } from "./ui";
import { WorkspaceStatusModal } from "./WorkspaceStatusModal";

export function AppShell({ children }: { children: ReactNode }) {
  const { ui, setUi } = useStore();
  const [configOpen, setConfigOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [statusOpen, setStatusOpen] = useState(false);

  return (
    <div className="app-page app-shell">
      {!ui.bannerClosed && (
        <div className="activity-bar">
          <strong className="activity-pill">
            <Sparkles size={15} />
            本地创作
          </strong>
          <span>Seedance 2.0 mock 已接入 · OpenAI 文本与图像走本地配置 · 纯本地创作链路</span>
          <IconButton label="隐藏活动条" onClick={() => setUi((value) => ({ ...value, bannerClosed: true }))}>
            <X size={16} />
          </IconButton>
        </div>
      )}
      <header className="topbar">
        <Link to="/" className="brand" aria-label="返回首页">
          <span className="brand-mark">TV</span>
          <span>LibTV</span>
          <small>Local</small>
        </Link>
        <nav className="topbar-nav" aria-label="主导航">
          <Link to="/project">项目</Link>
          <button type="button" className="plain-link">创作者挑战赛</button>
          <button type="button" className="plain-link" onClick={() => setHelpOpen(true)}>
            本地帮助
          </button>
        </nav>
        <div className="topbar-actions">
          <IconButton label="系统配置" onClick={() => setConfigOpen(true)}>
            <Settings size={18} />
          </IconButton>
          <IconButton label="本地状态" onClick={() => setStatusOpen(true)}>
            <Archive size={18} />
          </IconButton>
        </div>
      </header>
      {children}
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
      {helpOpen && <HelpModal onClose={() => setHelpOpen(false)} />}
      {statusOpen && <WorkspaceStatusModal onClose={() => setStatusOpen(false)} />}
    </div>
  );
}

function HelpModal({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="帮助中心" onClose={onClose} width={560}>
      <HelpPanel />
    </Modal>
  );
}
