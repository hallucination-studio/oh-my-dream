// Bottom asset drawer: a collapsible grid of generated media. Collapsed by
// default to keep the canvas roomy; the header shows the asset count.

import { useState } from "react";
import { AssetCard } from "./AssetCard.tsx";
import type { Asset } from "../api/index.ts";
import "./drawer.css";

export function AssetDrawer({ assets, error }: { assets: Asset[]; error: string | null }) {
  const [open, setOpen] = useState(false);

  return (
    <section className={`drawer${open ? " is-open" : ""}`}>
      <button className="drawer__head" onClick={() => setOpen((v) => !v)}>
        <span className={`drawer__chevron${open ? " is-open" : ""}`} aria-hidden="true" />
        <span className="drawer__title">Library</span>
        <span className="drawer__count">{assets.length}</span>
      </button>

      {open && (
        <div className="drawer__body">
          {error ? (
            <p className="drawer__error">Couldn’t load the library: {error}</p>
          ) : assets.length === 0 ? (
            <p className="drawer__empty">No media yet. Run a workflow to fill your library.</p>
          ) : (
            <div className="drawer__grid">
              {assets.map((asset) => (
                <AssetCard key={asset.id} asset={asset} />
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
