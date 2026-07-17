import { useState } from "react";
import "./settings.css";

const SECTIONS = ["canvas", "storage", "about"] as const;

export function SettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [section, setSection] = useState<(typeof SECTIONS)[number]>("canvas");
  if (!open) return null;

  return (
    <div className="scrim" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog">
        <div className="dialog__head">
          <b>Settings</b>
          <button className="dialog__close" onClick={onClose} aria-label="Close">×</button>
        </div>
        <div className="dialog__body">
          <nav className="dialog__nav">
            {SECTIONS.map((value) => (
              <button
                key={value}
                className={`dialog__navitem${section === value ? " is-on" : ""}`}
                onClick={() => setSection(value)}
              >
                {value[0].toUpperCase() + value.slice(1)}
              </button>
            ))}
          </nav>
          <div className="dialog__panel">
            <p className="dialog__grp">Nothing here yet.</p>
          </div>
        </div>
        <div className="dialog__foot">
          <button className="dialog__done" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}
