// A single asset tile: media preview with kind badge and hover actions
// (jump to source). Draggable onto the canvas.

import type { AssetViewModel } from "./model.ts";
import "./assetCard.css";

export function AssetCard({
  asset,
  selected,
  onSelect,
  onJump,
}: {
  asset: AssetViewModel;
  selected: boolean;
  onSelect: () => void;
  onJump: () => void;
}) {
  const src = asset.thumbnailUrl ?? asset.fileUrl;
  return (
    <figure
      className={`ac${selected ? " is-selected" : ""}`}
      draggable
      onClick={onSelect}
      onDragStart={(e) => e.dataTransfer.setData("application/oh-asset", asset.id)}
    >
      <div className={`ac__prev ac__prev--${asset.kind}`}>
        {src && asset.kind !== "audio" ? (
          <img className="ac__img" src={src} alt={asset.kind} loading="lazy" />
        ) : (
          <span className="ac__glyph">{asset.kind === "audio" ? "♪" : asset.kind}</span>
        )}
        <span className="ac__kind">{asset.kind}</span>
        <div className="ac__ov">
          <button
            className="ac__act"
            onClick={(e) => {
              e.stopPropagation();
              onJump();
            }}
            aria-label="Jump to source node"
          >
            ↗
          </button>
        </div>
      </div>
      <figcaption className="ac__prompt">{asset.prompt ?? "Untitled"}</figcaption>
      <div className="ac__meta">
        <span className="ac__pj" />
        {asset.projectName ?? "—"}
        <span className="ac__dt">{formatTime(asset.createdAt)}</span>
      </div>
    </figure>
  );
}

function formatTime(unixSeconds: number): string {
  if (!unixSeconds) {
    return "just now";
  }
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
