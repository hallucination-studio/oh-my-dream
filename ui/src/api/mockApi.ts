// Mock backend — an in-browser stand-in implementing the same WorkflowApi as
// the real Tauri client, so the canvas stays fully interactive with no Rust or
// network (e.g. in a plain `vite dev` browser tab). `selectApi` picks this when
// not running inside a Tauri window.

import type {
  CapabilityRef,
  GenerationProfileForCapability,
  GenerationProviderSettingsActionDto,
  GenerationProviderSettingsDto,
  GenerationTaskDto,
  GenerationTaskListPageDto,
  GenerationTaskRequestKindDto,
  GenerationTaskStatusDto,
  GenerationTaskSummaryDto,
  Project,
  NodeCapabilityContractDto,
  WorkflowApi,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowNodePresentationDto,
  WorkflowRunDto,
  WorkflowRunEventPageDto,
  WorkflowReadinessDto,
  WorkflowRunScopeDto,
  WorkflowWithReadinessDto,
} from "./types.ts";
import {
  assistantDecideWorkflowChange,
  assistantGetPendingWorkflowChange,
  assistantSendMessage,
  observeAssistantPresentationEvents,
} from "./mockAssistant.ts";
import {
  mockAssetGet,
  mockAssetImport,
  mockAssetIssuePreview,
  mockAssetList,
  mockAssetPublish,
  mockAssetsForNode,
  mockPreviewFixture,
} from "./mockAssets.ts";
import nodeCapabilitiesFixture from "../__fixtures__/node_capabilities.json";
import generationProviderSettingsFixture from "../__fixtures__/generation_provider_settings.json";
import generationTasksFixture from "../__fixtures__/generation_tasks.json";
import generationTaskFixture from "../__fixtures__/generation_task.json";

const mockCanonicalWorkflows = new Map<string, WorkflowDto>();
const mockRuns = new Map<string, WorkflowRunDto>();
const mockRunEvents = new Map<
  string,
  import("./types.ts").DurableWorkflowRunEventDto[]
>();
const mockRunObservers = new Set<
  (event: import("./types.ts").DurableWorkflowRunEventDto) => void
>();
const MOCK_PROJECT_ID = "10000000-0000-4000-8000-000000000001";
const MOCK_PROJECT_NAME = "Mock Project";
const mockProjects = new Map<string, Project>([
  [MOCK_PROJECT_ID, mockProject(MOCK_PROJECT_ID, MOCK_PROJECT_NAME)],
]);
let mockGenerationProviderSettings = structuredClone(
  generationProviderSettingsFixture,
) as GenerationProviderSettingsDto;
const mockGenerationTasks = structuredClone(generationTasksFixture.tasks) as GenerationTaskSummaryDto[];
const mockGenerationTaskDetail = structuredClone(generationTaskFixture) as GenerationTaskDto;
const mockGenerationProfiles: Record<string, GenerationProfileForCapability> = {
  "image.generate_from_text@1.0": {
    profile_ref: "image.high_quality_general@1",
    display_name: "Fast image model (sample)",
    availability: availableProfileAvailability(),
  },
  "video.generate_from_image@1.0": {
    profile_ref: "video.cinematic_image_animation@1",
    display_name: "Fast video model (sample)",
    availability: availableProfileAvailability(),
  },
  "audio.synthesize_speech_from_text@1.0": {
    profile_ref: "speech.multilingual_narration@1",
    display_name: "Fast audio model (sample)",
    availability: availableProfileAvailability(),
  },
};

async function listProjects() {
  return [...mockProjects.values()];
}

async function createProject(name: string) {
  const project = mockProject(crypto.randomUUID(), name);
  mockProjects.set(project.id, project);
  return project;
}

async function getProject(id: string) {
  const project = mockProjects.get(id);
  if (!project) throw new Error(`Project ${id} was not found`);
  return project;
}

async function renameProject(project: Project, name: string) {
  const renamed = { ...project, name, revision: String(Number(project.revision) + 1) };
  mockProjects.set(renamed.id, renamed);
  return renamed;
}

async function openProject(id: string) {
  const project = await getProject(id);
  return {
    project,
    current_workflow_summary: null,
  };
}

async function nodeCapabilityList(): Promise<NodeCapabilityContractDto[]> {
  return structuredClone(nodeCapabilitiesFixture) as NodeCapabilityContractDto[];
}

async function generationProfileListForCapability(
  reference: CapabilityRef,
): Promise<GenerationProfileForCapability[]> {
  const profile = mockGenerationProfiles[`${reference.id}@${reference.version}`];
  return profile ? [structuredClone(profile)] : [];
}

function availableProfileAvailability() {
  return {
    state: "available" as const,
    reason: null,
    retry_after_epoch_ms: null,
    observed_at_epoch_ms: "0",
    expires_at_epoch_ms: "30000",
  };
}

async function generationProviderSettingsGet(): Promise<GenerationProviderSettingsDto> {
  return structuredClone(mockGenerationProviderSettings);
}

async function generationProviderSettingsApply(
  expectedSettingsRevision: string,
  action: GenerationProviderSettingsActionDto,
): Promise<GenerationProviderSettingsDto> {
  if (mockGenerationProviderSettings.settings_revision !== expectedSettingsRevision) {
    throw new Error("generation_provider_settings.revision_conflict");
  }
  const profile = mockGenerationProviderSettings.profiles.find(
    (candidate) =>
      candidate.profile_ref === action.profile_ref &&
      candidate.generation_kind === action.generation_kind,
  );
  if (!profile) throw new Error("generation_provider_settings.invalid_request");
  const nextBinding = action.kind === "remove_binding"
    ? null
    : { provider_id: action.provider_id, route_id: action.route_id };
  if (nextBinding !== null && !profile.provider_choices.some((provider) =>
    provider.provider_id === nextBinding.provider_id &&
    provider.routes.some((route) => route.route_id === nextBinding.route_id)
  )) {
    throw new Error("generation_provider_settings.invalid_request");
  }
  if (JSON.stringify(profile.selected_binding) === JSON.stringify(nextBinding)) {
    return structuredClone(mockGenerationProviderSettings);
  }
  profile.selected_binding = nextBinding;
  mockGenerationProviderSettings.settings_revision = String(
    BigInt(mockGenerationProviderSettings.settings_revision) + 1n,
  );
  return structuredClone(mockGenerationProviderSettings);
}

async function generationTaskGet(projectId: string, taskId: string): Promise<GenerationTaskDto> {
  const task = mockGenerationTasks.find(
    (candidate) => candidate.project_id === projectId && candidate.id === taskId,
  );
  if (!task) throw new Error("generation_task.not_found");
  return structuredClone(task.id === mockGenerationTaskDetail.id
    ? mockGenerationTaskDetail
    : { ...task, result: null });
}

async function generationTaskList(
  projectId: string,
  status: GenerationTaskStatusDto | null = null,
  requestKind: GenerationTaskRequestKindDto | null = null,
  cursor: string | null = null,
  limit = 100,
): Promise<GenerationTaskListPageDto> {
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) {
    throw new Error("generation_task.invalid_request");
  }
  const offset = cursor === null ? 0 : parseCursor(cursor);
  const matching = mockGenerationTasks.filter((task) =>
    task.project_id === projectId &&
    (status === null || task.status === status) &&
    (requestKind === null || task.request_kind === requestKind),
  );
  const tasks = matching.slice(offset, offset + limit);
  const nextCursor = offset + limit < matching.length ? `mock:${offset + limit}` : null;
  return { tasks: structuredClone(tasks), next_cursor: nextCursor };
}

function parseCursor(cursor: string): number {
  const offset = Number(cursor.startsWith("mock:") ? cursor.slice(5) : NaN);
  if (!Number.isInteger(offset) || offset < 0) throw new Error("generation_task.invalid_request");
  return offset;
}

async function workflowCreate(projectId: string): Promise<WorkflowDto> {
  const existing = mockCanonicalWorkflows.get(projectId);
  if (existing) return structuredClone(existing);
  const now = String(Date.now());
  const workflow: WorkflowDto = {
    schema_version: 1,
    workflow_id: crypto.randomUUID(),
    project_id: projectId,
    revision: "1",
    created_at_epoch_ms: now,
    updated_at_epoch_ms: now,
    nodes: [],
    input_bindings: [],
  };
  mockCanonicalWorkflows.set(projectId, workflow);
  return structuredClone(workflow);
}

async function workflowGetCurrent(projectId: string): Promise<WorkflowWithReadinessDto> {
  const workflow = mockCanonicalWorkflows.get(projectId);
  if (!workflow) throw new Error("workflow.not_found");
  return { workflow: structuredClone(workflow), readiness: computeMockReadiness(workflow) };
}

async function workflowApplyMutation(
  projectId: string,
  workflowId: string,
  baseRevision: string,
  actions: WorkflowMutationActionDto[],
): Promise<WorkflowWithReadinessDto> {
  const current = (await workflowGetCurrent(projectId)).workflow;
  if (current.workflow_id !== workflowId || current.revision !== baseRevision) {
    throw new Error("workflow.revision_conflict");
  }
  const workflow = structuredClone(current);
  for (const action of actions) applyCanonicalAction(workflow, action);
  workflow.revision = String(Number(workflow.revision) + 1);
  workflow.updated_at_epoch_ms = String(Date.now());
  mockCanonicalWorkflows.set(projectId, workflow);
  return { workflow: structuredClone(workflow), readiness: computeMockReadiness(workflow) };
}

/** Canonical-minimum mock readiness: required inputs bound and required parameters set. */
function computeMockReadiness(workflow: WorkflowDto): WorkflowReadinessDto {
  const contracts = nodeCapabilitiesFixture as NodeCapabilityContractDto[];
  const issues: import("./types.ts").JsonObject[] = [];
  for (const node of workflow.nodes) {
    const contract = contracts.find(
      (candidate) => candidate.capability_ref.id === node.capability_id,
    );
    if (!contract) continue;
    for (const input of contract.inputs) {
      if (input.binding.kind !== "required_single_value") continue;
      const bound = workflow.input_bindings.some(
        (binding) =>
          binding.target_node_id === node.node_id &&
          binding.input_key === input.key &&
          binding.items.length > 0,
      );
      if (!bound) {
        issues.push({
          kind: "required_input_missing",
          node_id: node.node_id,
          detail: { input_key: input.key },
        });
      }
    }
    for (const parameter of contract.parameters) {
      if (parameter.presence.kind !== "required") continue;
      const value = node.parameters.find((candidate) => candidate.key === parameter.key)?.value;
      const present =
        value !== undefined && !(value.kind === "text" && value.value.trim() === "");
      if (!present) {
        issues.push({
          kind: "required_parameter_missing",
          node_id: node.node_id,
          detail: { parameter_key: parameter.key },
        });
      }
    }
  }
  return issues.length > 0 ? { state: "blocked", issues } : { state: "ready" };
}

async function workflowCheckReadiness(
  projectId: string,
  workflowId: string,
) {
  const current = await workflowGetCurrent(projectId);
  if (current.workflow.workflow_id !== workflowId) throw new Error("workflow.not_found");
  return current.readiness;
}

async function workflowStartRun(
  projectId: string,
  workflowId: string,
  workflowRevision: string,
  scope: WorkflowRunScopeDto,
): Promise<WorkflowRunDto> {
  const current = await workflowGetCurrent(projectId);
  if (current.readiness.state === "blocked") {
    const first = current.readiness.issues[0];
    const kind =
      typeof first === "object" && first !== null && !Array.isArray(first)
        ? String((first as import("./types.ts").JsonObject).kind)
        : "unknown";
    throw new Error(`workflow.not_ready:${kind}`);
  }
  const workflow = current.workflow;
  const now = String(Date.now());
  const run: WorkflowRunDto = {
    workflow_run_id: crypto.randomUUID(),
    project_id: projectId,
    workflow_id: workflowId,
    workflow_revision: workflowRevision,
    scope,
    state: "queued",
    created_at_epoch_ms: now,
    updated_at_epoch_ms: now,
    node_executions: workflow.nodes.map((node) => ({
      node_id: node.node_id,
      node_execution_id: crypto.randomUUID(),
      state: "pending",
      progress_basis_points: null,
    })),
  };
  mockRuns.set(run.workflow_run_id, run);
  mockRunEvents.set(run.workflow_run_id, []);
  queueMicrotask(() => executeMockRun(run.workflow_run_id));
  return structuredClone(run);
}

async function workflowCancelRun(
  _projectId: string,
  workflowRunId: string,
): Promise<WorkflowRunDto> {
  const run = requireMockRun(workflowRunId);
  // Cancelling a terminal run is rejected, mirroring the real backend.
  if (run.state !== "queued" && run.state !== "running") {
    throw new Error(`workflow.run_already_terminal: ${run.state}`);
  }
  const cancelled = { ...run, state: "cancelled" as const, updated_at_epoch_ms: String(Date.now()) };
  mockRuns.set(workflowRunId, cancelled);
  appendMockRunEvent(workflowRunId, { type: "run_cancelled" });
  return structuredClone(cancelled);
}

async function workflowGetRun(
  _projectId: string,
  workflowRunId: string,
): Promise<WorkflowRunDto> {
  return structuredClone(requireMockRun(workflowRunId));
}

async function workflowListRunEvents(
  _projectId: string,
  workflowRunId: string,
  afterSequence: string | null = null,
  limit = 500,
): Promise<WorkflowRunEventPageDto> {
  const after = BigInt(afterSequence ?? "0");
  const matching = (mockRunEvents.get(workflowRunId) ?? []).filter(
    (event) => BigInt(event.sequence) > after,
  );
  const events = matching.slice(0, limit);
  return {
    events: structuredClone(events),
    next_sequence:
      matching.length > limit ? events.at(-1)?.sequence ?? null : null,
  };
}

async function observeWorkflowRunEvents(
  observer: (event: import("./types.ts").DurableWorkflowRunEventDto) => void,
): Promise<() => void> {
  mockRunObservers.add(observer);
  return () => mockRunObservers.delete(observer);
}

async function workflowGetNodePresentation(
  projectId: string,
  workflowId: string,
  nodeId: string,
): Promise<WorkflowNodePresentationDto> {
  const workflow = (await workflowGetCurrent(projectId)).workflow;
  if (workflow.workflow_id !== workflowId) throw new Error("workflow.not_found");
  const node = workflow.nodes.find((candidate) => candidate.node_id === nodeId);
  if (!node) throw new Error("workflow.node_not_found");
  const kind = nodeKind(node);
  const latest = latestMockExecution(projectId, nodeId);
  if (kind === "text") {
    const text = mockTextParam(node);
    return {
      node_id: nodeId,
      current_revision: workflow.revision,
      capability_id: node.capability_id,
      capability_version: node.capability_version,
      readiness: computeMockReadiness(workflow),
      latest_execution: latest,
      presentation: { kind: "text", value: text ? [{ kind: "literal" as const, value: text }] : null },
    };
  }
  const asset = mockAssetsForNode(projectId, nodeId).at(-1) ?? null;
  return {
    node_id: nodeId,
    current_revision: workflow.revision,
    capability_id: node.capability_id,
    capability_version: node.capability_version,
    readiness: computeMockReadiness(workflow),
    latest_execution: latest,
    presentation: {
      kind,
      value: asset
        ? {
            asset_id: asset.asset_id,
            content_fingerprint_hex: asset.content.content_fingerprint_hex,
          }
        : null,
      preview_uri: asset ? mockPreviewFixture(kind, asset.display_name) : null,
    },
  };
}

function nodeKind(node: WorkflowDto["nodes"][number]): "image" | "video" | "audio" | "text" {
  return node.capability_id.startsWith("image.")
    ? "image"
    : node.capability_id.startsWith("video.")
      ? "video"
      : node.capability_id.startsWith("audio.")
        ? "audio"
        : "text";
}

function latestMockExecution(
  projectId: string,
  nodeId: string,
): WorkflowNodePresentationDto["latest_execution"] {
  const entry = [...mockRuns.values()]
    .filter((run) => run.project_id === projectId)
    .sort((a, b) => Number(b.created_at_epoch_ms) - Number(a.created_at_epoch_ms))
    .flatMap((run) => run.node_executions.map((execution) => ({ run, execution })))
    .find(({ execution }) => execution.node_id === nodeId);
  if (!entry) return null;
  const workflow = mockCanonicalWorkflows.get(projectId);
  return {
    workflow_run_id: entry.run.workflow_run_id,
    node_execution_id: entry.execution.node_execution_id,
    state: entry.execution.state,
    progress_basis_points: entry.execution.progress_basis_points,
    producing_revision: entry.run.workflow_revision,
    is_stale: workflow ? entry.run.workflow_revision !== workflow.revision : false,
    failure: null,
    block_reason: null,
  };
}

function requireMockRun(id: string): WorkflowRunDto {
  const run = mockRuns.get(id);
  if (!run) throw new Error("workflow_run.not_found");
  return run;
}

const MOCK_STEP_MS = 120;

function executeMockRun(runId: string): void {
  void runMockRunSteps(runId);
}

async function runMockRunSteps(runId: string): Promise<void> {
  const run = requireMockRun(runId);
  if (run.state === "cancelled") return;
  appendMockRunEvent(runId, { type: "run_queued" });
  appendMockRunEvent(runId, { type: "run_started" });
  transitionMockRun(runId, "running");
  const workflow = mockCanonicalWorkflows.get(run.project_id);
  if (!workflow) return;
  for (const execution of run.node_executions) {
    if (mockRunCancelled(runId)) return;
    const node = workflow.nodes.find((candidate) => candidate.node_id === execution.node_id);
    if (!node) continue;
    updateMockExecution(runId, execution.node_execution_id, "running", 0);
    appendMockRunEvent(runId, {
      type: "node_started",
      node_execution_id: execution.node_execution_id,
    });
    await mockDelay();
    if (mockRunCancelled(runId)) return;
    if (isMockGenerationCapability(node.capability_id)) {
      updateMockExecution(runId, execution.node_execution_id, "running", 4200);
      appendMockRunEvent(runId, {
        type: "node_progressed",
        node_execution_id: execution.node_execution_id,
        progress_basis_points: 4200,
      });
      await mockDelay();
      if (mockRunCancelled(runId)) return;
    }
    const outputs = produceMockOutputs(run, workflow, node);
    updateMockExecution(runId, execution.node_execution_id, "succeeded", 10_000);
    appendMockRunEvent(runId, {
      type: "node_succeeded",
      node_execution_id: execution.node_execution_id,
      outputs,
    });
  }
  transitionMockRun(runId, "succeeded");
  appendMockRunEvent(runId, { type: "run_succeeded" });
}

function mockDelay(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, MOCK_STEP_MS));
}

function mockRunCancelled(runId: string): boolean {
  return mockRuns.get(runId)?.state === "cancelled";
}

function transitionMockRun(runId: string, state: WorkflowRunDto["state"]): void {
  const run = mockRuns.get(runId);
  if (!run) return;
  mockRuns.set(runId, { ...run, state, updated_at_epoch_ms: String(Date.now()) });
}

function updateMockExecution(
  runId: string,
  executionId: string,
  state: "pending" | "running" | "succeeded",
  progressBasisPoints: number,
): void {
  const run = mockRuns.get(runId);
  if (!run) return;
  mockRuns.set(runId, {
    ...run,
    updated_at_epoch_ms: String(Date.now()),
    node_executions: run.node_executions.map((execution) =>
      execution.node_execution_id === executionId
        ? { ...execution, state, progress_basis_points: progressBasisPoints }
        : execution,
    ),
  });
}

function isMockGenerationCapability(capabilityId: string): boolean {
  return (
    capabilityId === "image.generate_from_text" ||
    capabilityId === "video.generate_from_image" ||
    capabilityId === "audio.synthesize_speech_from_text"
  );
}

function produceMockOutputs(
  run: WorkflowRunDto,
  workflow: WorkflowDto,
  node: WorkflowDto["nodes"][number],
): Array<{ key: string; value: import("./types.ts").JsonObject }> {
  const capabilityId = node.capability_id;
  if (capabilityId === "text.provide_literal") {
    return [{ key: "text", value: { type: "text", value: mockTextParam(node) ?? "" } }];
  }
  if (capabilityId.endsWith(".read_asset")) {
    const kind = capabilityId.split(".")[0] ?? "image";
    const assetParam = node.parameters.find((param) => param.key === "asset_id")?.value;
    return [{
      key: kind,
      value: {
        type: kind,
        asset_id: assetParam?.kind === "managed_asset" ? assetParam.asset_id : "",
      },
    }];
  }
  if (isMockGenerationCapability(capabilityId)) {
    const kind = capabilityId.split(".")[0] as "image" | "video" | "audio";
    const displayName = mockPromptFor(workflow, node) ?? `Generated ${kind}`;
    const asset = mockAssetPublish(run.project_id, kind, displayName, {
      kind: "workflow_node_output",
      workflow_node_id: node.node_id,
      workflow_run_id: run.workflow_run_id,
    });
    return [{
      key: kind,
      value: {
        type: kind,
        asset_id: asset.asset_id,
        preview_uri: mockPreviewFixture(kind, asset.display_name),
      },
    }];
  }
  return [];
}

function mockTextParam(node: WorkflowDto["nodes"][number]): string | null {
  const param = node.parameters.find((candidate) => candidate.key === "text")?.value;
  return param?.kind === "text" && param.value.trim() ? param.value.trim().slice(0, 80) : null;
}

function mockPromptFor(
  workflow: WorkflowDto,
  node: WorkflowDto["nodes"][number],
): string | null {
  for (const binding of workflow.input_bindings) {
    if (binding.target_node_id !== node.node_id) continue;
    if (binding.input_key !== "prompt" && binding.input_key !== "text") continue;
    const source = workflow.nodes.find(
      (candidate) => candidate.node_id === binding.items[0]?.source_node_id,
    );
    const text = source ? mockTextParam(source) : null;
    if (text) return text;
  }
  return null;
}

function appendMockRunEvent(
  runId: string,
  payload: import("./types.ts").JsonObject & { type: string },
): void {
  const events = mockRunEvents.get(runId) ?? [];
  const event = {
    workflow_run_id: runId,
    sequence: String(events.length + 1),
    occurred_at_epoch_ms: String(Date.now()),
    payload,
  };
  events.push(event);
  mockRunEvents.set(runId, events);
  for (const observer of mockRunObservers) observer(structuredClone(event));
}

function applyCanonicalAction(workflow: WorkflowDto, action: WorkflowMutationActionDto): void {
  if (action.kind === "add_node") {
    workflow.nodes.push({
      node_id: action.node_id,
      capability_id: action.capability.id,
      capability_version: action.capability.version,
      parameters: action.parameters,
      canvas_position: action.canvas_position,
    });
  } else if (action.kind === "remove_node") {
    workflow.nodes = workflow.nodes.filter((node) => node.node_id !== action.node_id);
    workflow.input_bindings = workflow.input_bindings.filter(
      (binding) =>
        binding.target_node_id !== action.node_id &&
        binding.items.every((item) => item.source_node_id !== action.node_id),
    );
  } else if (action.kind === "replace_node_parameters") {
    requireCanonicalNode(workflow, action.node_id).parameters = action.parameters;
  } else if (action.kind === "select_node_capability") {
    const node = requireCanonicalNode(workflow, action.node_id);
    node.capability_id = action.capability.id;
    node.capability_version = action.capability.version;
    node.parameters = action.parameters;
  } else if (action.kind === "move_node") {
    requireCanonicalNode(workflow, action.node_id).canvas_position = action.canvas_position;
  } else if (action.kind === "bind_single_input") {
    workflow.input_bindings = workflow.input_bindings.filter(
      (binding) =>
        binding.target_node_id !== action.target.node_id ||
        binding.input_key !== action.target.input_key,
    );
    workflow.input_bindings.push({
      target_node_id: action.target.node_id,
      input_key: action.target.input_key,
      kind: "single",
      items: [{
        input_item_id: action.item.input_item_id,
        source_node_id: action.item.source_node_id,
        source_output_key: action.item.source_output_key,
        input_role_key: action.item.input_role_key,
      }],
    });
  } else if (action.kind === "remove_input_item") {
    for (const binding of workflow.input_bindings) {
      if (
        binding.target_node_id === action.target.node_id &&
        binding.input_key === action.target.input_key
      ) {
        binding.items = binding.items.filter((item) => item.input_item_id !== action.input_item_id);
      }
    }
    workflow.input_bindings = workflow.input_bindings.filter(
      (binding) => binding.items.length > 0,
    );
  }
}

function requireCanonicalNode(workflow: WorkflowDto, id: string) {
  const node = workflow.nodes.find((candidate) => candidate.node_id === id);
  if (!node) throw new Error("workflow.node_not_found");
  return node;
}

function mockProject(id: string, name: string): Project {
  return {
    id,
    name,
    revision: "1",
    created_at_epoch_ms: "0",
    updated_at_epoch_ms: "0",
  };
}


export const mockApi: WorkflowApi = {
  assetImport: mockAssetImport,
  assetGet: mockAssetGet,
  assetList: mockAssetList,
  assetIssuePreview: mockAssetIssuePreview,
  listProjects,
  createProject,
  getProject,
  renameProject,
  openProject,
  nodeCapabilityList,
  generationProfileListForCapability,
  generationProviderSettingsGet,
  generationProviderSettingsApply,
  generationTaskGet,
  generationTaskList,
  workflowCreate,
  workflowGetCurrent,
  workflowApplyMutation,
  workflowCheckReadiness,
  workflowStartRun,
  workflowCancelRun,
  workflowGetRun,
  workflowListRunEvents,
  observeWorkflowRunEvents,
  workflowGetNodePresentation,
  assistantSendMessage,
  assistantGetPendingWorkflowChange,
  assistantDecideWorkflowChange,
  observeAssistantPresentationEvents,
};
