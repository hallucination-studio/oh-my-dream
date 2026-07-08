// Left icon rail — switches the side panel between the node library and the
// asset library.

import "./rail.css";

export type RailTab = "nodes" | "assets";

export function IconRail({ tab, onSelect }: { tab: RailTab; onSelect: (t: RailTab) => void }) {
  return (
    <div className="rail glass">
      <button
        className={`rail__btn${tab === "nodes" ? " is-on" : ""}`}
        onClick={() => onSelect("nodes")}
        aria-label="Nodes"
        title="Nodes"
      >
        ⬢
      </button>
      <button
        className={`rail__btn${tab === "assets" ? " is-on" : ""}`}
        onClick={() => onSelect("assets")}
        aria-label="Assets"
        title="Assets"
      >
        ▦
      </button>
    </div>
  );
}
