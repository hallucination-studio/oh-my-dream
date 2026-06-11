import { SlidersHorizontal } from "lucide-react";
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
          <span>Oh My Dream</span>
        </Link>
        <div className="topbar-actions">
          <IconButton label="打开设置" className="settings-trigger" onClick={() => setConfigOpen(true)}>
            <SlidersHorizontal size={17} />
          </IconButton>
        </div>
      </header>
      {children}
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
    </div>
  );
}
