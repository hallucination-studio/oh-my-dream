// Left icon rail — switches the side panel between the node library and the
// asset library.

import "./rail.css";

export type RailTab = "nodes" | "assets";

export function IconRail({ tab, onSelect }: { tab: RailTab; onSelect: (t: RailTab) => void }) {
  return (
    <div className="rail">
      <button
        className={`rail__btn${tab === "nodes" ? " is-on" : ""}`}
        onClick={() => onSelect("nodes")}
        aria-label="Nodes"
        title="Nodes"
      >
        <NodesIcon />
      </button>
      <button
        className={`rail__btn${tab === "assets" ? " is-on" : ""}`}
        onClick={() => onSelect("assets")}
        aria-label="Assets"
        title="Assets"
      >
        <AssetsIcon />
      </button>
    </div>
  );
}

function NodesIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <rect x="3" y="3" width="7" height="7" rx="1.5" />
      <rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="8.5" y="14" width="7" height="7" rx="1.5" />
    </svg>
  );
}

function AssetsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <rect x="3" y="3" width="18" height="18" rx="2.5" />
      <circle cx="8.5" cy="8.5" r="1.8" />
      <path d="M21 15l-5-5L5 21" />
    </svg>
  );
}
