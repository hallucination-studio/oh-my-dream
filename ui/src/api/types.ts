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

export type GenerationKind = "text" | "image" | "video" | "voice";

export interface GenerationProviderSettingsBindingDto {
  provider_id: string;
  route_id: string;
}

export interface GenerationProviderSettingsRouteChoiceDto {
  route_id: string;
  display_name: string;
}

export interface GenerationProviderSettingsProviderChoiceDto {
  provider_id: string;
  display_name: string;
  routes: GenerationProviderSettingsRouteChoiceDto[];
}

export interface GenerationProviderSettingsProfileDto {
  profile_ref: string;
  generation_kind: GenerationKind;
  selected_binding: GenerationProviderSettingsBindingDto | null;
  provider_choices: GenerationProviderSettingsProviderChoiceDto[];
}

export interface GenerationProviderSettingsDto {
  settings_revision: string;
  profiles: GenerationProviderSettingsProfileDto[];
}

export type GenerationProviderSettingsActionDto =
  | {
      kind: "set_binding";
      profile_ref: string;
      generation_kind: GenerationKind;
      provider_id: string;
      route_id: string;
    }
  | {
      kind: "remove_binding";
      profile_ref: string;
      generation_kind: GenerationKind;
    };

export interface GenerationProviderSettingsApplyRequestDto {
  expected_settings_revision: string;
  action: GenerationProviderSettingsActionDto;
}

export interface AssistantProviderSettingsDto {
  settings_revision: string;
  enabled: boolean;
  base_url: string;
  model_id: string | null;
  has_api_key: boolean;
}

export interface AssistantProviderModelsDto {
  models: string[];
}

export type GenerationTaskStatusDto =
  | "queued"
  | "running"
  | "cancel_requested"
  | "succeeded"
  | "failed"
  | "cancelled";

export type GenerationTaskRequestKindDto = "text" | "image" | "video" | "voice";

export type GenerationTaskFailureKindDto =
  | "invalid_request"
  | "authentication"
  | "permission_denied"
  | "content_policy"
  | "rate_limited"
  | "provider_unavailable"
  | "timeout"
  | "provider_rejected"
  | "invalid_provider_response"
  | "ambiguous_submission"
  | "input_asset_unavailable"
  | "output_asset_import"
  | "internal";

export interface GenerationTaskFailureDto {
  kind: GenerationTaskFailureKindDto;
  code: string;
  message: string;
}

export type GenerationTaskResultDto =
  | { kind: "text"; content: string }
  | { kind: "asset"; asset_id: string; media_kind: "image" | "video" | "audio" };

export interface GenerationTaskSummaryDto {
  id: string;
  project_id: string;
  workflow_id: string;
  workflow_run_id: string;
  workflow_node_id: string;
  workflow_node_execution_id: string;
  request_kind: GenerationTaskRequestKindDto;
  status: GenerationTaskStatusDto;
  progress_percent: number | null;
  generation_profile_ref: string;
  provider_id: string;
  provider_display_name: string | null;
  prompt_preview: string | null;
  preview_asset_id: string | null;
  has_result: boolean;
  failure: GenerationTaskFailureDto | null;
  created_at_epoch_ms: string;
  updated_at_epoch_ms: string;
  completed_at_epoch_ms: string | null;
}

export interface GenerationTaskDto extends GenerationTaskSummaryDto {
  result: GenerationTaskResultDto | null;
}

export interface GenerationTaskListPageDto {
  tasks: GenerationTaskSummaryDto[];
  next_cursor: string | null;
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
  generationProviderSettingsGet: () => Promise<GenerationProviderSettingsDto>;
  generationProviderSettingsApply: (
    expectedSettingsRevision: string,
    action: GenerationProviderSettingsActionDto,
  ) => Promise<GenerationProviderSettingsDto>;
  assistantProviderSettingsGet: () => Promise<AssistantProviderSettingsDto>;
  assistantProviderModelsList: (
    baseUrl: string,
    apiKey: string | null,
  ) => Promise<AssistantProviderModelsDto>;
  assistantProviderSettingsTestAndApply: (
    expectedSettingsRevision: string,
    baseUrl: string,
    apiKey: string | null,
    modelId: string,
  ) => Promise<AssistantProviderSettingsDto>;
  assistantProviderSettingsDisable: (
    expectedSettingsRevision: string,
  ) => Promise<AssistantProviderSettingsDto>;
  generationTaskGet: (projectId: string, taskId: string) => Promise<GenerationTaskDto>;
  generationTaskList: (
    projectId: string,
    status?: GenerationTaskStatusDto | null,
    requestKind?: GenerationTaskRequestKindDto | null,
    cursor?: string | null,
    limit?: number,
  ) => Promise<GenerationTaskListPageDto>;
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
