// Loads assets from the backend seam and exposes a refresh trigger. Used by
// the asset drawer; refreshed after a successful run so freshly generated
// media appears.

import { useCallback, useEffect, useState } from "react";
import { api, type Asset } from "../api/index.ts";

export function useAssets() {
  const [assets, setAssets] = useState<Asset[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setAssets(await api.listAssets());
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
