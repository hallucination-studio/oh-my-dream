// The backend API contract shared by the mock and the real Tauri client.
//
// The App talks only to this interface, so switching backends is a one-line
// change in `selectApi`. Method shapes mirror the src-tauri commands:
// run_workflow / list_assets / get_asset / assets_root.

import type {
  NodeProgressEvent,
  RunLifecycleStatus,
  RunOutputs,
  RunProgress,
  Workflow,
} from "../workflow/types.ts";

export type AssetKind = "image" | "video" | "audio";
export type AssetSort = "newest" | "oldest" | "cost_desc" | "cost_asc";

/** Metadata for a stored asset, mirroring the backend AssetDto. */
export interface AssetDto {
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
  seed: string | null; // Decimal u64; never decode through a JavaScript number.
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

export interface AssistantSkills {
  installed: string[];
  enabled: string[];
}

export interface AssistantConfig {
  enabled: boolean;
  base_url: string;
  model: string;
  has_key: boolean;
  temperature: number;
  max_tool_iters: number;
  system_prompt_extra: string | null;
  developer_mode: boolean;
  skills: AssistantSkills;
}

export interface AssistantConfigInput {
  enabled: boolean;
  base_url: string;
  model: string;
  api_key: string | null;
  clear_api_key: boolean;
  temperature: number;
  max_tool_iters: number;
  system_prompt_extra: string | null;
  developer_mode: boolean;
  enabled_skills: string[];
}

export interface Capability {
  name: string;
  description: string;
  kind: "backend" | "ui";
  command: string | null;
  parameters: unknown;
  returns: unknown;
  confirm: boolean;
}

export interface CapabilityManifest {
  capabilities: Capability[];
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | JsonObject;

export interface JsonObject {
  [key: string]: JsonValue;
}

export type AssistantOperationEffect =
  | "local_read"
  | "visible_reversible_workflow_patch"
  | "prepared_approval_execution";

export interface AssistantOperationContract {
  id: string;
  version: number;
  description: string;
  effect: AssistantOperationEffect;
  strict_json_schema: boolean;
  needs_approval: boolean;
  input_schema: JsonObject;
  output_schema: JsonObject;
}

export interface AssistantOperationsFixture {
  operations: AssistantOperationContract[];
}

export interface AssistantSession {
  port: number;
  token: string;
}

export interface Skill {
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  developer_mode_required: boolean;
  status: string;
}

export interface ListAssetsOptions {
  kind?: AssetKind;
  project_id?: string;
  model?: string;
  prompt?: string;
  sort?: AssetSort;
}

export type WorkflowRunEvent =
  | { event: "started"; run_id: string; project_id: string }
  | { event: "progress"; run_id: string; node: NodeProgressEvent };

export type WorkflowRunResult =
  | { status: "succeeded"; run_id: string; outputs: RunOutputs }
  | { status: "cancelled"; run_id: string }
  | { status: "failed"; run_id: string; reason: string };

export type CancelWorkflowRunResult =
  | { status: "requested"; run_id: string }
  | { status: "not_active"; run_id: string };

/** A handle allowing the caller to cancel an in-flight run. */
export interface RunHandle {
  runId: string;
  cancel: () => void;
}

/**
 * Separates node progress from workflow lifecycle transitions.
 * Committed `done` or `cached` progress may arrive while cancellation is pending.
 */
export interface RunObserver {
  onProgress: (progress: RunProgress) => void;
  onStatus: (status: RunLifecycleStatus) => void;
}

export interface WorkflowApi {
  /** Runs a workflow, streaming node progress and lifecycle transitions to `observe`. */
  runWorkflow: (workflow: Workflow, observe: RunObserver) => RunHandle;
  /** Returns the backend asset root when one exists. */
  assetsRoot: () => Promise<string | null>;
  /** Lists stored assets, optionally filtered by kind. */
  listAssets: (options?: ListAssetsOptions) => Promise<AssetDto[]>;
  /** Fetches a single asset by id. */
  getAsset: (id: string) => Promise<AssetDto>;
  listProjects: () => Promise<Project[]>;
  createProject: (name: string) => Promise<Project>;
  openProject: (id: string) => Promise<ProjectWorkspace>;
  saveWorkflow: (workflow: Workflow) => Promise<void>;
  loadWorkflow: (projectId: string) => Promise<Workflow>;
  getProviders: () => Promise<Provider[]>;
  setActiveProvider: (providerId: string) => Promise<void>;
  setProviderKey: (providerId: string, key: string) => Promise<void>;
  getAssistantConfig: () => Promise<AssistantConfig>;
  setAssistantConfig: (input: AssistantConfigInput) => Promise<void>;
  getAssistantSession: () => Promise<AssistantSession>;
  getCapabilityManifest: () => Promise<CapabilityManifest>;
  listSkills: () => Promise<Skill[]>;
  installSkill: (path: string) => Promise<Skill>;
  setSkillEnabled: (name: string, enabled: boolean) => Promise<void>;
  uninstallSkill: (name: string) => Promise<void>;
}
