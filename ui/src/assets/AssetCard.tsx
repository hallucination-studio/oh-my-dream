// A single asset tile: media preview with kind badge and hover actions
// (jump to source). Draggable onto the canvas.

import type { AssetViewModel } from "./model.ts";
import { AssetMediaPreview } from "./AssetMediaPreview.tsx";
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
  return (
    <figure
      className={`ac${selected ? " is-selected" : ""}`}
      draggable
      onClick={onSelect}
      onDragStart={(e) => e.dataTransfer.setData("application/oh-asset", asset.id)}
    >
      <div className={`ac__prev ac__prev--${asset.kind}`}>
        <AssetMediaPreview
          asset={asset}
          className={asset.previewUrl ? "ac__img" : "ac__glyph"}
        />
        <span className="ac__kind">{asset.kind}</span>
        {asset.sourceNodeId && (
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
        )}
      </div>
      <figcaption className="ac__prompt">{asset.displayName}</figcaption>
      <div className="ac__meta">
        <span className="ac__dt">{formatTime(asset.createdAtEpochMs)}</span>
      </div>
    </figure>
  );
}

function formatTime(epochMs: string): string {
  if (epochMs === "0") {
    return "just now";
  }
  return new Date(Number(epochMs)).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
