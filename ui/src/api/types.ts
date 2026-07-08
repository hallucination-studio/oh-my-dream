// The backend API contract shared by the mock and the real Tauri client.
//
// The App talks only to this interface, so switching backends is a one-line
// change in `selectApi`. Method shapes mirror the src-tauri commands:
// run_workflow / list_assets / get_asset / assets_root.

import type { RunStatus, Workflow } from "../workflow/types.ts";

export type AssetKind = "image" | "video" | "audio";
export type AssetSort = "newest" | "oldest" | "cost_desc" | "cost_asc";

/** Metadata for a stored asset, mirroring the backend AssetDto. */
export interface Asset {
  id: string;
  kind: AssetKind;
  file_path: string;
  thumbnail_path: string | null;
  workflow_snapshot: unknown;
  prompt: string | null;
  project_id: string | null;
  project_name: string | null;
  source_node_id: string | null;
  source_node_type: string | null;
  model: string | null;
  seed: number | null;
  cost: number | null;
  tags: string[];
  created_at: number;
}

export interface Project {
  id: string;
  name: string;
  created_at: number;
}

export interface ProjectWorkspace {
  project: Project;
  workflow_json: Workflow;
}

export interface Provider {
  id: string;
  name: string;
  active: boolean;
  has_key: boolean;
}

export interface ListAssetsOptions {
  kind?: AssetKind;
  project_id?: string;
  model?: string;
  prompt?: string;
  sort?: AssetSort;
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
  listAssets: (options?: ListAssetsOptions) => Promise<Asset[]>;
  /** Fetches a single asset by id. */
  getAsset: (id: string) => Promise<Asset>;
  listProjects: () => Promise<Project[]>;
  createProject: (name: string) => Promise<Project>;
  openProject: (id: string) => Promise<ProjectWorkspace>;
  saveWorkflow: (workflow: Workflow) => Promise<void>;
  loadWorkflow: (projectId: string) => Promise<Workflow>;
  getProviders: () => Promise<Provider[]>;
  setActiveProvider: (providerId: string) => Promise<void>;
  setProviderKey: (providerId: string, key: string) => Promise<void>;
}
