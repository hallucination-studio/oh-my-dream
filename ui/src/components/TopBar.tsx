// Top bar: brand mark, workflow name, and the primary Run / Cancel control
// with a live run indicator.

import type { RunStatus } from "../workflow/types.ts";
import "./topbar.css";

export function TopBar({
  status,
  onRun,
  onCancel,
}: {
  status: RunStatus;
  onRun: () => void;
  onCancel: () => void;
}) {
  const running = status.state === "running";
  return (
    <header className="topbar">
      <div className="topbar__brand">
        <span className="topbar__mark" aria-hidden="true" />
        <span className="topbar__name">oh-my-dream</span>
        <span className="topbar__doc">untitled workflow</span>
      </div>

      <div className="topbar__actions">
        <RunIndicator status={status} />
        {running ? (
          <button className="topbar__btn topbar__btn--cancel" onClick={onCancel}>
            Cancel
          </button>
        ) : (
          <button className="topbar__btn topbar__btn--run" onClick={onRun}>
            <span className="topbar__play" aria-hidden="true" />
            Run
          </button>
        )}
      </div>
    </header>
  );
}

function RunIndicator({ status }: { status: RunStatus }) {
  switch (status.state) {
    case "idle":
      return null;
    case "running":
      return <span className="topbar__state topbar__state--run">Running {status.nodeId}…</span>;
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
