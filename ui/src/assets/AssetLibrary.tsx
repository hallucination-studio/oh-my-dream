// Full asset library view (rail-switched): professional search, kind filters
// with counts, grid/list modes, and a detail panel. Assets are draggable onto
// the canvas and can jump to their source.

import { useMemo, useState } from "react";
import type { AssetKind } from "../api/index.ts";
import type { AssetViewModel } from "./model.ts";
import { AssetCard } from "./AssetCard.tsx";
import { AssetDetail } from "./AssetDetail.tsx";
import "./library.css";

export function AssetLibrary({
  assets,
  error,
  loading = false,
  hasProject = true,
  onAddToCanvas,
  onJumpToNode,
  selectedAssetId,
  onSelectAsset,
  onImport,
}: {
  assets: AssetViewModel[];
  error: string | null;
  loading?: boolean;
  hasProject?: boolean;
  onAddToCanvas: (asset: AssetViewModel) => void;
  onJumpToNode: (asset: AssetViewModel) => void;
  selectedAssetId?: string | null;
  onSelectAsset?: (assetId: string | null) => void;
  onImport: (kind: AssetKind) => void;
}) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<AssetKind | "all">("all");
  const [mode, setMode] = useState<"grid" | "list">("grid");
  const [localSelectedId, setLocalSelectedId] = useState<string | null>(null);
  const selectedId = selectedAssetId === undefined ? localSelectedId : selectedAssetId;
  const setSelectedId = onSelectAsset ?? setLocalSelectedId;

  const counts = useMemo(() => {
    const next = { all: assets.length, image: 0, video: 0, audio: 0 };
    for (const asset of assets) next[asset.kind] += 1;
    return next;
  }, [assets]);

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
            <button
              className="lib__mode"
              onClick={() => setMode((current) => (current === "grid" ? "list" : "grid"))}
              aria-label={mode === "grid" ? "Switch to list view" : "Switch to grid view"}
              title={mode === "grid" ? "List view" : "Grid view"}
            >
              {mode === "grid" ? "≣" : "▦"}
            </button>
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
              placeholder="Search by prompt or name…"
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="lib__filters" role="group" aria-label="Filter by kind">
            {(["all", "image", "video", "audio"] as const).map((k) => (
              <button
                key={k}
                className={`lib__chip${kind === k ? " is-on" : ""}`}
                onClick={() => setKind(k)}
              >
                {k === "all" ? "All" : k[0]!.toUpperCase() + k.slice(1)} ({counts[k]})
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

        <div className={mode === "grid" ? "lib__grid" : "lib__list"} aria-busy={loading}>
          {error ? (
            <p className="lib__msg lib__msg--err">{error}</p>
          ) : !hasProject ? (
            <p className="lib__msg">Open a Project to see its Assets.</p>
          ) : loading ? (
            <SkeletonTiles mode={mode} />
          ) : assets.length === 0 ? (
            <p className="lib__msg">Run a workflow or import media to fill your library.</p>
          ) : filtered.length === 0 ? (
            <p className="lib__msg">
              No assets match this filter.{" "}
              <button
                className="lib__clear"
                onClick={() => {
                  setQuery("");
                  setKind("all");
                }}
              >
                Clear filter
              </button>
            </p>
          ) : mode === "grid" ? (
            filtered.map((asset) => (
              <AssetCard
                key={asset.id}
                asset={asset}
                selected={asset.id === selectedId}
                onSelect={() => setSelectedId(asset.id)}
                onJump={() => onJumpToNode(asset)}
              />
            ))
          ) : (
            filtered.map((asset) => (
              <button
                key={asset.id}
                className={`lib__row${asset.id === selectedId ? " is-on" : ""}`}
                draggable
                onDragStart={(e) => e.dataTransfer.setData("application/oh-asset", asset.id)}
                onClick={() => setSelectedId(asset.id)}
              >
                <span className={`lib__rowkind lib__rowkind--${asset.kind}`}>{asset.kind}</span>
                <span className="lib__rowname">{asset.displayName}</span>
                <span className="lib__rowfacts">{asset.facts ?? ""}</span>
              </button>
            ))
          )}
        </div>
      </section>

      {selected ? (
        <AssetDetail asset={selected} onAddToCanvas={onAddToCanvas} onJumpToNode={onJumpToNode} />
      ) : null}
    </>
  );
}

function SkeletonTiles({ mode }: { mode: "grid" | "list" }) {
  const count = mode === "grid" ? 8 : 5;
  return (
    <>
      {Array.from({ length: count }, (_, index) => (
        <div key={index} className={`lib__skel${mode === "list" ? " lib__skel--row" : ""}`} aria-hidden="true" />
      ))}
      <span className="lib__msg" role="status">Loading library…</span>
    </>
  );
}
