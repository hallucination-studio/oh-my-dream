// The backend API contract shared by the mock and the real Tauri client.
//
// The App talks only to this interface, so switching backends is a one-line
// change in `selectApi`. Method shapes mirror the src-tauri commands:
// run_workflow / list_assets / get_asset / assets_root.

import type { RunStatus, Workflow } from "../workflow/types.ts";

/** Metadata for a stored asset, mirroring the backend AssetDto. */
export interface Asset {
  id: string;
  kind: "image" | "video";
  file_path: string;
  thumbnail_path: string | null;
  workflow_snapshot: unknown;
  source_node_id: string | null;
  tags: string[];
  created_at: number;
}

/** A handle allowing the caller to cancel an in-flight run. */
export interface RunHandle {
  cancel: () => void;
}

/** Callback invoked with each status transition during a run. */
export type RunObserver = (status: RunStatus) => void;

export interface WorkflowApi {
  /** Runs a workflow, streaming status transitions to `observe`. */
  runWorkflow: (workflow: Workflow, observe: RunObserver) => RunHandle;
  /** Returns the backend asset root when one exists. */
  assetsRoot: () => Promise<string | null>;
  /** Lists stored assets, optionally filtered by kind. */
  listAssets: (kind?: "image" | "video") => Promise<Asset[]>;
  /** Fetches a single asset by id. */
  getAsset: (id: string) => Promise<Asset>;
}
