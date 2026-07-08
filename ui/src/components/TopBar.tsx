// Top bar: brand, project switcher, run state, settings gear, Run/Cancel.

import type { Project } from "../api/index.ts";
import type { RunStatus } from "../workflow/types.ts";
import "./topbar.css";

export function TopBar({
  project,
  status,
  onOpenProjects,
  onOpenSettings,
  onRun,
  onCancel,
}: {
  project: Project | null;
  status: RunStatus;
  onOpenProjects: () => void;
  onOpenSettings: () => void;
  onRun: () => void;
  onCancel: () => void;
}) {
  const running = status.state === "running";
  return (
    <header className="topbar">
      <div className="topbar__brand">
        <BrandMark />
        <b>oh&#8209;my&#8209;dream</b>
      </div>

      <span className="topbar__sep" aria-hidden="true" />
      <button className="topbar__proj" onClick={onOpenProjects}>
        <span className="topbar__pdot" />
        <span className="topbar__pn">{project?.name ?? "No project"}</span>
        <span className="topbar__car" aria-hidden="true">▾</span>
      </button>

      <div className="topbar__spacer" />
      <RunState status={status} />
      <span className="topbar__sep" aria-hidden="true" />
      <button className="topbar__gear" onClick={onOpenSettings} aria-label="Settings">
        <GearIcon />
      </button>
      {running ? (
        <button className="topbar__run topbar__run--cancel" onClick={onCancel}>Cancel</button>
      ) : (
        <button className="topbar__run" onClick={onRun}>
          <span className="topbar__play" aria-hidden="true" />
          Run
        </button>
      )}
    </header>
  );
}

// Geometric brand mark: a dark rounded square with an aperture play triangle.
function BrandMark() {
  return (
    <svg className="topbar__mark" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="1.2" y="1.2" width="21.6" height="21.6" rx="6.4" fill="#14161d" />
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="#fff" />
    </svg>
  );
}

function GearIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function RunState({ status }: { status: RunStatus }) {
  switch (status.state) {
    case "idle":
      return null;
    case "running":
      return (
        <span className="topbar__state">
          <span className="topbar__spin" aria-hidden="true" />
          Running · {status.nodeId}…
        </span>
      );
    case "succeeded":
      return (
        <span className="topbar__state topbar__state--ok">
          Done · {Object.keys(status.outputs).length} outputs
        </span>
      );
    case "failed":
      return <span className="topbar__state topbar__state--err">{status.reason}</span>;
  }
}
