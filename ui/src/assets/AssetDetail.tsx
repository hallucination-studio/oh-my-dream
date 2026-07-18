// Detail panel for the selected asset: large preview, prompt, source metadata
// with jump links, and actions (export, add to canvas).

import type { AssetViewModel } from "./model.ts";
import { AssetMediaPreview } from "./AssetMediaPreview.tsx";
import "./assetDetail.css";

export function AssetDetail({
  asset,
  onAddToCanvas,
  onJumpToNode,
}: {
  asset: AssetViewModel | null;
  onAddToCanvas: (asset: AssetViewModel) => void;
  onJumpToNode: (asset: AssetViewModel) => void;
}) {
  if (!asset) {
    return (
      <aside className="adet">
        <div className="adet__empty">Select an asset to see its details.</div>
      </aside>
    );
  }

  return (
    <aside className="adet">
      <div className={`adet__prev adet__prev--${asset.kind}`}>
        <AssetMediaPreview
          asset={asset}
          className={asset.previewUrl ? "adet__img" : "adet__glyph"}
        />
      </div>

      <div className="adet__body">
        <span className={`adet__kind adet__kind--${asset.kind}`}>{asset.kind}</span>
        <p className="adet__prompt">{asset.displayName}</p>

        <Row k="Details" v={asset.facts ?? "—"} mono />
        {asset.sourceNodeType && (
          <Row
            k="From node"
            v={
              <button className="adet__link adet__link--btn" onClick={() => onJumpToNode(asset)}>
                {asset.sourceNodeType} ↗
              </button>
            }
          />
        )}
        <Row k="Created" v={formatTime(asset.createdAtEpochMs)} mono />
      </div>

      <div className="adet__actions">
        <button className="adet__btn adet__btn--pri" onClick={() => onAddToCanvas(asset)}>
          ＋ Add to canvas
        </button>
      </div>
    </aside>
  );
}

function Row({ k, v, mono }: { k: string; v: React.ReactNode; mono?: boolean }) {
  return (
    <div className="adet__row">
      <span className="adet__k">{k}</span>
      <span className={`adet__v${mono ? " is-mono" : ""}`}>{v}</span>
    </div>
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
