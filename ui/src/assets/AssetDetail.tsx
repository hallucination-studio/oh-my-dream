// Detail panel for the selected asset: large preview, prompt, source metadata
// with jump links, and actions (export, add to canvas).

import type { Asset } from "../api/index.ts";
import "./assetDetail.css";

export function AssetDetail({
  asset,
  onAddToCanvas,
  onJumpToNode,
}: {
  asset: Asset | null;
  onAddToCanvas: (asset: Asset) => void;
  onJumpToNode: (asset: Asset) => void;
}) {
  if (!asset) {
    return (
      <aside className="adet glass">
        <div className="adet__empty">Select an asset to see its details.</div>
      </aside>
    );
  }

  const src = asset.thumbnail_path ?? asset.file_path;
  return (
    <aside className="adet glass">
      <div className={`adet__prev adet__prev--${asset.kind}`}>
        {src && asset.kind !== "audio" ? (
          <img className="adet__img" src={src} alt={asset.kind} />
        ) : (
          <span className="adet__glyph">{asset.kind === "audio" ? "♪" : asset.kind}</span>
        )}
      </div>

      <div className="adet__body">
        <span className={`adet__kind adet__kind--${asset.kind}`}>{asset.kind}</span>
        <p className="adet__prompt">{asset.prompt ?? "Untitled"}</p>

        <Row k="Model" v={asset.model ?? "—"} mono />
        {asset.seed != null && <Row k="Seed" v={String(asset.seed)} mono />}
        {asset.cost != null && <Row k="Cost" v={`$${(asset.cost / 1_000_000).toFixed(4)}`} mono />}
        {asset.project_name && (
          <Row k="Project" v={<span className="adet__link">{asset.project_name} ↗</span>} />
        )}
        {asset.source_node_type && (
          <Row
            k="From node"
            v={
              <button className="adet__link adet__link--btn" onClick={() => onJumpToNode(asset)}>
                {asset.source_node_type} ↗
              </button>
            }
          />
        )}
        <Row k="Created" v={formatTime(asset.created_at)} mono />
      </div>

      <div className="adet__actions">
        <button className="adet__btn">Export</button>
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
