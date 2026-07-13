// Loads assets from the backend seam and exposes a refresh trigger. Used by
// the asset drawer; refreshed after a successful run so freshly generated
// media appears.

import { useCallback, useEffect, useState } from "react";
import { api } from "../api/index.ts";
import { assetFromDto, type AssetViewModel } from "./model.ts";

export function useAssets() {
  const [assets, setAssets] = useState<AssetViewModel[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setAssets((await api.listAssets()).map(assetFromDto));
      setError(null);
    } catch (cause) {
      // Surface the failure to the drawer rather than silently showing empty.
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { assets, error, refresh };
}
