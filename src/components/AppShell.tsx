import { Settings } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { ConfigModal } from "./ConfigModal";
import { IconButton } from "./ui";

export function AppShell({ children }: { children: ReactNode }) {
  const [configOpen, setConfigOpen] = useState(false);

  return (
    <div className="app-page app-shell">
      <header className="topbar canvas-shell-topbar">
        <Link to="/" className="brand" aria-label="返回首页">
          <span className="brand-mark">TV</span>
          <span>LibTV</span>
          <small>Local</small>
        </Link>
        <nav className="topbar-nav" aria-label="主导航">
          <Link to="/project">项目</Link>
        </nav>
        <div className="topbar-actions">
          <IconButton label="系统配置" onClick={() => setConfigOpen(true)}>
            <Settings size={18} />
          </IconButton>
        </div>
      </header>
      {children}
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
    </div>
  );
}
