// A single asset tile: square media preview with the kind badge, hover to
// reveal the created time. Mirrors ComfyUI's media card (rounded, aspect
// square, hover affordances) in our own palette.

import type { Asset } from "../api/index.ts";
import "./assetCard.css";

export function AssetCard({ asset }: { asset: Asset }) {
  const src = asset.thumbnail_path ?? asset.file_path;
  return (
    <figure className="asset-card" data-kind={asset.kind}>
      <div className="asset-card__preview">
        {src ? (
          <img className="asset-card__img" src={src} alt={`${asset.kind} asset`} loading="lazy" />
        ) : (
          <div className="asset-card__placeholder" />
        )}
        <span className={`asset-card__kind asset-card__kind--${asset.kind}`}>{asset.kind}</span>
      </div>
      <figcaption className="asset-card__meta">{formatTime(asset.created_at)}</figcaption>
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
