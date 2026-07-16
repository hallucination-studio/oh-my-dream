// The backend API contract shared by the mock and the real Tauri client.
//
// The App talks only to this interface, so switching backends is a one-line
// change in `selectApi`. Method shapes mirror the src-tauri commands:
// run_workflow / list_assets / get_asset / assets_root.

import type {
  NodeProgressEvent,
  PortType,
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
  revision: string;
  created_at_epoch_ms: string;
  updated_at_epoch_ms: string;
}

export interface ProjectWorkflowSummary {
  workflow_id: string;
  workflow_revision: string;
  readiness: "ready" | "blocked";
}

export interface NodeCapabilityContractDto {
  capability_ref: CapabilityRef;
  parameters: Array<{ key: string; constraint: Record<string, unknown>; presence: Record<string, unknown> }>;
  inputs: Array<{ key: string; binding: Record<string, unknown> }>;
  outputs: Array<{ key: string; data_type: "text" | "image" | "video" | "audio"; is_primary: boolean }>;
  execution_kind:
    | "pure_value"
    | "managed_asset_read"
    | "content_generation"
    | "media_transformation"
    | "content_analysis";
}

export interface GenerationProfileForCapability {
  profile_ref: string;
  display_name: string;
  availability: {
    state: "available" | "unavailable" | "indeterminate";
    reason: string | null;
    retry_after_epoch_ms: string | null;
    observed_at_epoch_ms: string;
    expires_at_epoch_ms: string;
  };
}

export interface WorkflowHead {
  project_id: string;
  revision: number;
  workflow: Workflow;
}

export type WorkflowNodeRef =
  | { kind: "id"; id: string }
  | { kind: "alias"; alias: string };

export interface WorkflowPatchOutputRef {
  node: WorkflowNodeRef;
  output: string;
}

export type WorkflowPatchBinding =
  | { kind: "single"; source: WorkflowPatchOutputRef }
  | { kind: "ordered_many"; sources: WorkflowPatchOutputRef[] };

export type WorkflowPatchOperation =
  | {
      op: "add_node";
      alias: string;
      capability: { id: string; version: string };
      params: Record<string, unknown>;
      position: [number, number] | null;
    }
  | { op: "replace_params"; node: WorkflowNodeRef; params: Record<string, unknown> }
  | { op: "set_input"; node: WorkflowNodeRef; input: string; binding: WorkflowPatchBinding }
  | { op: "clear_input"; node: WorkflowNodeRef; input: string }
  | { op: "remove_node"; node: WorkflowNodeRef }
  | { op: "set_position"; node: WorkflowNodeRef; position: [number, number] };

export interface WorkflowApplyPatchInput {
  expected_revision: number | null;
  operations: WorkflowPatchOperation[];
}

export interface WorkflowReadinessBlocker {
  code: string;
  pointer: string;
  constraint: string;
}

export type CapabilityRef = { id: string; version: string };
export type CapabilitySelector = { type_id: string; mode: string };

export type CapabilityCardinality =
  | "one"
  | { many: { minimum: number; maximum: number | null } };

export interface CapabilityPort {
  name: string;
  port_type: PortType;
  cardinality: CapabilityCardinality;
  required: boolean;
}

export type CapabilityEffect = "pure" | "local_read" | "external";

export interface CapabilityContract {
  reference: CapabilityRef;
  inputs: CapabilityPort[];
  outputs: CapabilityPort[];
  params_schema: JsonObject;
  default_params: Record<string, JsonValue> | null;
  contextual_creation: ContextualCreation | null;
  effects: CapabilityEffect[];
}

export interface ContextualCreation {
  route: string;
}

export interface CapabilityPresentation {
  label: string;
  description: string;
  category: string;
  search_terms: string[];
}

export type CapabilityAvailability = "available" | "unavailable" | "degraded";

export interface CapabilityStatus {
  availability: CapabilityAvailability;
  reason: string | null;
  provider_health: string | null;
  status_revision: number;
}

export interface CapabilitySummary {
  selector: CapabilitySelector;
  reference: CapabilityRef;
  presentation: CapabilityPresentation;
  contextual_creation: ContextualCreation | null;
  status: CapabilityStatus;
}

export interface CapabilityBundle {
  selector: CapabilitySelector | null;
  reference: CapabilityRef;
  contract: CapabilityContract | null;
  presentation: CapabilityPresentation | null;
  status: CapabilityStatus;
}

export interface CapabilityCatalogEntry {
  selector: CapabilitySelector;
  contract: CapabilityContract;
  presentation: CapabilityPresentation;
  status: CapabilityStatus;
}

export interface CapabilityCatalog {
  capabilities: CapabilityCatalogEntry[];
}

export interface CapabilitySearchRequest {
  query: string;
  category?: string | null;
  type_id?: string | null;
  cursor?: string | null;
  limit?: number;
}

export interface CapabilitySearchPage {
  capabilities: CapabilitySummary[];
  next_cursor: string | null;
}

export interface CapabilityBundles {
  capabilities: CapabilityBundle[];
}

export interface WorkflowApplyPatchOutput {
  workflow_head: WorkflowHead | null;
  aliases: Array<{ alias: string; node_id: string }>;
  readiness_blockers: WorkflowReadinessBlocker[];
  changed: boolean;
  deduplicated: boolean;
  undo_id: string | null;
}

export interface ProjectWorkspace {
  project: Project;
  current_workflow_summary: ProjectWorkflowSummary | null;
  /** Removed by V3 when the canonical Workflow command slice activates. */
  workflow_head: WorkflowHead | null;
}

export type OpenProjectResult = ProjectWorkspace;

export interface Provider {
  id: string;
  name: string;
  active: boolean;
  has_key: boolean;
}

export interface AssistantConfig {
  enabled: boolean;
  base_url: string;
  model: string;
  has_key: boolean;
}

export interface AssistantConfigInput {
  enabled: boolean;
  base_url: string;
  model: string;
  api_key: string | null;
  clear_api_key: boolean;
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

export interface AssistantContext {
  project_id: string | null;
  workflow_present: boolean;
  workflow_revision: number | null;
  selected_node_ids: string[];
  selected_asset_ids: string[];
}

export interface AssistantSendInput {
  project_id: string;
  workflow_present: boolean;
  workflow_revision: number | null;
  selected_node_ids: string[];
  selected_asset_ids: string[];
  text: string;
}

export interface AssistantApprovalDecisionInput {
  project_id: string;
  approval_scope_id: string;
  candidate_digest: string;
  approved: boolean;
}

export interface AssistantPendingApproval {
  project_id: string;
  approval_scope_id: string;
  user_intent: string;
  candidate_digest: string;
  reviewer_version: string;
  evidence_hash: string;
  review_summary: string;
  review_findings: string[];
  effect: "apply_reviewed_workflow_candidate";
  workflow: Workflow;
  readiness_blockers: JsonValue;
  assets: Array<{ asset_id: string; kind: AssetKind }>;
}

export type ResponsesStreamEvent = JsonObject & { type: string };

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
  getProject: (id: string) => Promise<Project>;
  renameProject: (project: Project, name: string) => Promise<Project>;
  openProject: (id: string) => Promise<ProjectWorkspace>;
  nodeCapabilityList: () => Promise<NodeCapabilityContractDto[]>;
  generationProfileListForCapability: (
    reference: CapabilityRef,
  ) => Promise<GenerationProfileForCapability[]>;
  searchCapabilities: (request: CapabilitySearchRequest) => Promise<CapabilitySearchPage>;
  getCapabilityBundles: (refs: CapabilityRef[]) => Promise<CapabilityBundles>;
  applyWorkflowPatch: (
    projectId: string,
    requestId: string,
    input: WorkflowApplyPatchInput,
  ) => Promise<WorkflowApplyPatchOutput>;
  getProviders: () => Promise<Provider[]>;
  setActiveProvider: (providerId: string) => Promise<void>;
  setProviderKey: (providerId: string, key: string) => Promise<void>;
  getAssistantConfig: () => Promise<AssistantConfig>;
  setAssistantConfig: (input: AssistantConfigInput) => Promise<void>;
  sendAssistant: (
    input: AssistantSendInput,
    onEvent: (event: ResponsesStreamEvent) => void,
  ) => Promise<WorkflowHead | null>;
  getPendingAssistantApproval: (projectId: string) => Promise<AssistantPendingApproval | null>;
  decideAssistantApproval: (
    input: AssistantApprovalDecisionInput,
    onEvent: (event: ResponsesStreamEvent) => void,
  ) => Promise<WorkflowHead | null>;
}
