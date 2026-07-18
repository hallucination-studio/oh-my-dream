// Top bar: brand, project switcher, run state, settings gear, Run/Cancel.

import type { Project } from "../api/index.ts";
import type { RunStatus } from "../workflow/types.ts";
import type { ProjectWorkspaceState } from "../workflow/useProjectWorkspace.ts";
import "./topbar.css";

export function TopBar({
  project,
  status,
  workspaceState,
  onOpenProjects,
  onOpenSettings,
  onRun,
  onCancel,
  onOpenRunDetails,
  hasRunDetails,
  runDisabled = false,
  runDisabledReason = null,
  runNodeLabel = (nodeId: string) => nodeId,
  saving = null,
}: {
  project: Project | null;
  status: RunStatus;
  workspaceState: ProjectWorkspaceState;
  onOpenProjects: () => void;
  onOpenSettings: () => void;
  onRun: () => void;
  onCancel: () => void;
  onOpenRunDetails: () => void;
  hasRunDetails: boolean;
  runDisabled?: boolean;
  runDisabledReason?: string | null;
  runNodeLabel?: (nodeId: string) => string;
  saving?: boolean | null;
}) {
  const running = status.state === "running";
  const cancelling = status.state === "cancelling";
  const cancelFailed = status.state === "cancel_failed";
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
      {saving !== null && (
        <span className="topbar__save" role="status">
          {saving ? "Saving…" : "Saved"}
        </span>
      )}

      <div className="topbar__spacer" />
      <WorkspaceState state={workspaceState} />
      <RunState status={status} runNodeLabel={runNodeLabel} />
      {hasRunDetails && (
        <button className="topbar__details" onClick={onOpenRunDetails} aria-label="Run details" title="Run details">
          <RunDetailsIcon />
        </button>
      )}
      <span className="topbar__sep" aria-hidden="true" />
      <button className="topbar__gear" onClick={onOpenSettings} aria-label="Settings">
        <GearIcon />
      </button>
      {running ? (
        <button className="topbar__run topbar__run--cancel" onClick={onCancel}>Cancel</button>
      ) : cancelling ? (
        <button className="topbar__run topbar__run--cancel" disabled aria-busy="true">
          Cancelling…
        </button>
      ) : cancelFailed ? (
        <button className="topbar__run topbar__run--cancel" onClick={onCancel}>
          Retry Cancel
        </button>
      ) : (
        <button
          className="topbar__run"
          onClick={onRun}
          disabled={runDisabled}
          title={runDisabledReason ?? undefined}
        >
          <span className="topbar__play" aria-hidden="true" />
          Run all
        </button>
      )}
    </header>
  );
}

function WorkspaceState({ state }: { state: ProjectWorkspaceState }) {
  if (state.state === "ready" || state.state === "no_project") {
    return null;
  }
  const text =
    state.state === "booting"
      ? "Loading projects…"
      : state.state === "opening"
        ? "Opening project…"
        : `Project unavailable · ${state.reason}`;
  return (
    <span className="topbar__state topbar__state--err" role="status" aria-live="polite">
      {text}
    </span>
  );
}

// Geometric brand mark: a dark rounded square with an aperture play triangle.
function BrandMark() {
  return (
    <svg className="topbar__mark" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="1.2" y="1.2" width="21.6" height="21.6" rx="6.4" fill="var(--accent)" />
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="var(--on-accent)" />
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

function RunDetailsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <path d="M5 6.5h14M5 12h14M5 17.5h9" />
      <circle cx="18" cy="17.5" r="1.5" />
    </svg>
  );
}

function RunState({
  status,
  runNodeLabel,
}: {
  status: RunStatus;
  runNodeLabel: (nodeId: string) => string;
}) {
  switch (status.state) {
    case "idle":
      return null;
    case "running":
      return (
        <span className="topbar__state" role="status">
          <span className="topbar__spin" aria-hidden="true" />
          {`Running · ${runNodeLabel(status.nodeId)} · ${Math.round(status.progress * 100)}%`}
        </span>
      );
    case "cancelling":
      return <span className="topbar__state" role="status">Cancelling…</span>;
    case "cancel_failed":
      return (
        <span className="topbar__state topbar__state--err" role="status">
          Cancel request failed · {status.reason}
        </span>
      );
    case "cancelled":
      return <span className="topbar__state" role="status">Cancelled</span>;
    case "succeeded": {
      const assets = Object.values(status.outputs).reduce(
        (count, outputs) =>
          count +
          Object.values(outputs).filter(
            (output) =>
              output.kind === "image" || output.kind === "video" || output.kind === "audio",
          ).length,
        0,
      );
      const stepsLabel = `${status.steps} ${status.steps === 1 ? "step" : "steps"} complete`;
      const assetsLabel =
        assets > 0 ? ` · ${assets} ${assets === 1 ? "asset" : "assets"} created` : "";
      return (
        <span className="topbar__state topbar__state--ok" role="status">
          {stepsLabel}
          {assetsLabel}
        </span>
      );
    }
    case "failed":
      return (
        <span className="topbar__state topbar__state--err" role="status">
          {status.reason}
        </span>
      );
  }
}
