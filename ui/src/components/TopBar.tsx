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
    <header className="topbar glass">
      <div className="topbar__brand">
        <span className="topbar__mark" aria-hidden="true" />
        <b>oh-my-dream</b>
      </div>

      <button className="topbar__proj" onClick={onOpenProjects}>
        <span className="topbar__pdot" />
        <span className="topbar__pn">{project?.name ?? "No project"}</span>
        <span className="topbar__car" aria-hidden="true">▾</span>
      </button>

      <div className="topbar__spacer" />
      <RunState status={status} />
      <button className="topbar__gear" onClick={onOpenSettings} aria-label="Settings">⚙</button>
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

function RunState({ status }: { status: RunStatus }) {
  switch (status.state) {
    case "idle":
      return null;
    case "running":
      return <span className="topbar__state">Running · {status.nodeId}…</span>;
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
