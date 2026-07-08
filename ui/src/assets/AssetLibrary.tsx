// Full asset library view (rail-switched): professional search, kind/project
// filters, a big grid, and a detail panel. Assets are draggable onto the
// canvas and can jump to their source.

import { useMemo, useState } from "react";
import type { Asset, AssetKind } from "../api/index.ts";
import { AssetCard } from "./AssetCard.tsx";
import { AssetDetail } from "./AssetDetail.tsx";
import "./library.css";

export function AssetLibrary({
  assets,
  error,
  onAddToCanvas,
  onJumpToNode,
}: {
  assets: Asset[];
  error: string | null;
  onAddToCanvas: (asset: Asset) => void;
  onJumpToNode: (asset: Asset) => void;
}) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<AssetKind | "all">("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return assets.filter((a) => {
      if (kind !== "all" && a.kind !== kind) {
        return false;
      }
      if (q && !(a.prompt ?? "").toLowerCase().includes(q) && !(a.model ?? "").toLowerCase().includes(q)) {
        return false;
      }
      return true;
    });
  }, [assets, query, kind]);

  const selected = filtered.find((a) => a.id === selectedId) ?? null;

  return (
    <>
      <section className="lib glass">
        <div className="lib__top">
          <div className="lib__title-row">
            <span className="lib__title">Library</span>
            <span className="lib__count">{filtered.length}</span>
          </div>
          <div className="lib__search">
            <span aria-hidden="true">⌕</span>
            <input
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
        </div>

        <div className="lib__grid">
          {error ? (
            <p className="lib__msg lib__msg--err">Couldn’t load the library: {error}</p>
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
