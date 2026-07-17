// The backend API contract shared by the mock and the real Tauri client.
//
// The App talks only to this interface, so switching backends is a one-line
// change in `selectApi`. Method shapes mirror the src-tauri commands:
// Canonical command slices plus the Asset and Assistant surfaces pending hard cut.

import type {
  PortType,
  Workflow,
} from "../workflow/types.ts";
import type {
  DurableWorkflowRunEventDto,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowNodePresentationDto,
  WorkflowReadinessDto,
  WorkflowRunDto,
  WorkflowRunEventPageDto,
  WorkflowRunScopeDto,
  WorkflowWithReadinessDto,
} from "./workflowTypes.ts";
export type * from "./workflowTypes.ts";
import type { AssetDto, AssetKind, AssetListPageDto, AssetPreviewDto } from "./assetTypes.ts";
export type * from "./assetTypes.ts";

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

export interface ProjectWorkspace {
  project: Project;
  current_workflow_summary: ProjectWorkflowSummary | null;
}

export type OpenProjectResult = ProjectWorkspace;

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
  | "authoritative_read"
  | "assistant_state_mutation"
  | "human_approval_request";

export interface AssistantOperationContract {
  id: string;
  description: string;
  effect: AssistantOperationEffect;
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
  workflow_revision: string | null;
  selected_node_ids: string[];
  selected_asset_ids: string[];
  text: string;
}

export interface AssistantApprovalDecisionInput {
  project_id: string;
  workflow_change_id: string;
  approval_scope_id: string;
  mutation_digest_hex: string;
  decision: "approve" | "reject";
}

export interface AssistantPendingWorkflowChange {
  workflow_change_id: string;
  project_id: string;
  base_workflow_revision: string;
  mutation_digest_hex: string;
  approval_scope_id: string;
  expires_at_epoch_ms: string;
  state:
    | "proposed"
    | "review_rejected"
    | "awaiting_approval"
    | "rejected"
    | "applying"
    | "applied"
    | "apply_failed"
    | "expired";
  lineage:
    | { kind: "user_message"; invocation_id: string; intent: string }
    | { kind: "reviewed_repair"; activation_id: string; failed_workflow_run_id: string };
  mutations: JsonValue[];
  readiness_issues: JsonValue[];
}

export interface AssistantSendMessageResult {
  invocation_id: string;
  final_text: string;
}

export interface AssistantWorkflowChangeDecisionResult {
  workflow_change_id: string;
  state: AssistantPendingWorkflowChange["state"];
}

export type AssistantPresentationEvent =
  & { invocation_id: string; sequence: string }
  & (
    | { kind: "text_delta"; text: string }
    | { kind: "tool_activity"; tool_id: string; state: "started" | "completed" | "failed" }
    | { kind: "workflow_change_ready"; workflow_change_id: string }
    | { kind: "invocation_completed" }
    | { kind: "invocation_failed"; error: { code: string; message: string } }
  );

export interface WorkflowApi {
  assetImport: (projectId: string, expectedMediaKind: AssetKind) => Promise<AssetDto | null>;
  assetGet: (projectId: string, assetId: string) => Promise<AssetDto>;
  assetList: (
    projectId: string,
    mediaKind?: AssetKind | null,
    cursor?: string | null,
    limit?: number,
  ) => Promise<AssetListPageDto>;
  assetIssuePreview: (projectId: string, assetId: string) => Promise<AssetPreviewDto>;
  listProjects: () => Promise<Project[]>;
  createProject: (name: string) => Promise<Project>;
  getProject: (id: string) => Promise<Project>;
  renameProject: (project: Project, name: string) => Promise<Project>;
  openProject: (id: string) => Promise<ProjectWorkspace>;
  nodeCapabilityList: () => Promise<NodeCapabilityContractDto[]>;
  generationProfileListForCapability: (
    reference: CapabilityRef,
  ) => Promise<GenerationProfileForCapability[]>;
  workflowCreate: (projectId: string) => Promise<WorkflowDto>;
  workflowGetCurrent: (projectId: string) => Promise<WorkflowWithReadinessDto>;
  workflowApplyMutation: (
    projectId: string,
    workflowId: string,
    baseRevision: string,
    actions: WorkflowMutationActionDto[],
  ) => Promise<WorkflowWithReadinessDto>;
  workflowCheckReadiness: (
    projectId: string,
    workflowId: string,
  ) => Promise<WorkflowReadinessDto>;
  workflowStartRun: (
    projectId: string,
    workflowId: string,
    workflowRevision: string,
    scope: WorkflowRunScopeDto,
  ) => Promise<WorkflowRunDto>;
  workflowCancelRun: (projectId: string, workflowRunId: string) => Promise<WorkflowRunDto>;
  workflowGetRun: (projectId: string, workflowRunId: string) => Promise<WorkflowRunDto>;
  workflowListRunEvents: (
    projectId: string,
    workflowRunId: string,
    afterSequence?: string | null,
    limit?: number,
  ) => Promise<WorkflowRunEventPageDto>;
  observeWorkflowRunEvents: (
    onEvent: (event: DurableWorkflowRunEventDto) => void,
  ) => Promise<() => void>;
  workflowGetNodePresentation: (
    projectId: string,
    workflowId: string,
    nodeId: string,
  ) => Promise<WorkflowNodePresentationDto>;
  assistantSendMessage: (input: AssistantSendInput) => Promise<AssistantSendMessageResult>;
  assistantGetPendingWorkflowChange: (
    projectId: string,
  ) => Promise<AssistantPendingWorkflowChange | null>;
  assistantDecideWorkflowChange: (
    input: AssistantApprovalDecisionInput,
  ) => Promise<AssistantWorkflowChangeDecisionResult>;
  observeAssistantPresentationEvents: (
    onEvent: (event: AssistantPresentationEvent) => void,
  ) => Promise<() => void>;
}
