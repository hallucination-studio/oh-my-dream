// Full asset library view (rail-switched): professional search, kind/project
// filters, a big grid, and a detail panel. Assets are draggable onto the
// canvas and can jump to their source.

import { useMemo, useState } from "react";
import type { AssetKind } from "../api/index.ts";
import type { AssetViewModel } from "./model.ts";
import { AssetCard } from "./AssetCard.tsx";
import { AssetDetail } from "./AssetDetail.tsx";
import "./library.css";

export function AssetLibrary({
  assets,
  error,
  onAddToCanvas,
  onJumpToNode,
  selectedAssetId,
  onSelectAsset,
  onImport,
}: {
  assets: AssetViewModel[];
  error: string | null;
  onAddToCanvas: (asset: AssetViewModel) => void;
  onJumpToNode: (asset: AssetViewModel) => void;
  selectedAssetId?: string | null;
  onSelectAsset?: (assetId: string | null) => void;
  onImport: (kind: AssetKind) => void;
}) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<AssetKind | "all">("all");
  const [localSelectedId, setLocalSelectedId] = useState<string | null>(null);
  const selectedId = selectedAssetId === undefined ? localSelectedId : selectedAssetId;
  const setSelectedId = onSelectAsset ?? setLocalSelectedId;

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return assets.filter((a) => {
      if (kind !== "all" && a.kind !== kind) {
        return false;
      }
      if (q && !a.displayName.toLowerCase().includes(q)) {
        return false;
      }
      return true;
    });
  }, [assets, query, kind]);

  const selected = filtered.find((a) => a.id === selectedId) ?? null;

  return (
    <>
      <section className="lib">
        <div className="lib__top">
          <div className="lib__title-row">
            <span className="lib__title">Library</span>
            <span className="lib__count">{filtered.length}</span>
          </div>
          <div className="lib__search">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="M21 21l-4-4" />
            </svg>
            <input
              id="asset-library-search"
              name="asset-library-search"
              aria-label="Search assets"
              value={query}
              placeholder="Search by prompt or model…"
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="lib__filters">
            {(["all", "image", "video", "audio"] as const).map((k) => (
              <button
                key={k}
                className={`lib__chip${kind === k ? " is-on" : ""}`}
                onClick={() => setKind(k)}
              >
                {k === "all" ? "All" : k[0].toUpperCase() + k.slice(1)}
              </button>
            ))}
          </div>
          <div className="lib__filters">
            {(["image", "video", "audio"] as const).map((assetKind) => (
              <button key={assetKind} className="lib__chip" onClick={() => onImport(assetKind)}>
                Import {assetKind}
              </button>
            ))}
          </div>
        </div>

        <div className="lib__grid">
          {error ? (
            <p className="lib__msg lib__msg--err">{error}</p>
          ) : filtered.length === 0 ? (
            <p className="lib__msg">No media yet. Run a workflow to fill your library.</p>
          ) : (
            filtered.map((asset) => (
              <AssetCard
                key={asset.id}
                asset={asset}
                selected={asset.id === selectedId}
                onSelect={() => setSelectedId(asset.id)}
                onJump={() => onJumpToNode(asset)}
              />
            ))
          )}
        </div>
      </section>

      <AssetDetail asset={selected} onAddToCanvas={onAddToCanvas} onJumpToNode={onJumpToNode} />
    </>
  );
}
