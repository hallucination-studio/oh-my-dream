// Loads assets from the backend seam and exposes a refresh trigger. Used by
// the asset drawer; refreshed after a successful run so freshly generated
// media appears.

import { useCallback, useEffect, useState } from "react";
import { api } from "../api/index.ts";
import { assetFromDto, type AssetViewModel } from "./model.ts";

export function useAssets(projectId: string | null) {
  const [assets, setAssets] = useState<AssetViewModel[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      if (!projectId) {
        setAssets([]);
        setError(null);
        return;
      }
      const listed = await api.assetList(projectId, null, null, 100);
      const projected = await Promise.all(listed.assets.map(async (asset) => {
        if (asset.content_state !== "available") return assetFromDto(asset, null);
        try {
          return assetFromDto(asset, await api.assetIssuePreview(projectId, asset.asset_id));
        } catch {
          return assetFromDto(asset, null);
        }
      }));
      setAssets(projected);
      setError(null);
    } catch (cause) {
      // Surface the failure to the drawer rather than silently showing empty.
      setError(String(cause));
    }
  }, [projectId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const importAsset = useCallback(async (kind: import("../api/types.ts").AssetKind) => {
    if (!projectId) return null;
    const imported = await api.assetImport(projectId, kind);
    if (imported) await refresh();
    return imported;
  }, [projectId, refresh]);

  return { assets, error, importAsset, refresh };
}
