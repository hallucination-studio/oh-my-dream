// Mock backend — an in-browser stand-in implementing the same WorkflowApi as
// the real Tauri client, so the canvas stays fully interactive with no Rust or
// network (e.g. in a plain `vite dev` browser tab). `selectApi` picks this when
// not running inside a Tauri window.

import type {
  AssetDto,
  AssistantConfig,
  AssistantConfigInput,
  CapabilityRef,
  GenerationProfileForCapability,
  Project,
  NodeCapabilityContractDto,
  WorkflowApi,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowNodePresentationDto,
  WorkflowRunDto,
  WorkflowRunEventPageDto,
  WorkflowRunScopeDto,
  WorkflowWithReadinessDto,
} from "./types.ts";
import {
  decideAssistantApproval,
  getPendingAssistantApproval,
  sendAssistant,
} from "./mockAssistant.ts";
import nodeCapabilitiesFixture from "../__fixtures__/node_capabilities.json";

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
let mockAssistantConfig: AssistantConfig = {
  enabled: false,
  base_url: "https://api.openai.com/v1",
  model: "gpt-5.4",
  has_key: false,
};

// The mock has no persistent store; asset listing is empty until a real backend
// is present. This keeps the interface total rather than throwing.
async function listAssets(): Promise<AssetDto[]> {
  return [];
}

async function assetsRoot(): Promise<string | null> {
  return null;
}

async function getAsset(id: string): Promise<AssetDto> {
  throw new Error(`Mock backend has no asset store; cannot fetch asset ${id}`);
}

async function listProjects() {
  return [...mockProjects.values()];
}

async function createProject(name: string) {
  const project = mockProject(MOCK_PROJECT_ID, name);
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
  _reference: CapabilityRef,
): Promise<GenerationProfileForCapability[]> {
  return [];
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
  return { workflow: structuredClone(workflow), readiness: { state: "ready" } };
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
  return { workflow: structuredClone(workflow), readiness: { state: "ready" } };
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
  const workflow = (await workflowGetCurrent(projectId)).workflow;
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
  const kind = node.capability_id.startsWith("image.")
    ? "image"
    : node.capability_id.startsWith("video.")
      ? "video"
      : node.capability_id.startsWith("audio.")
        ? "audio"
        : "text";
  return {
    node_id: nodeId,
    current_revision: workflow.revision,
    capability_id: node.capability_id,
    capability_version: node.capability_version,
    readiness: { state: "ready" },
    latest_execution: null,
    presentation: kind === "text"
      ? { kind: "text", value: null }
      : { kind, value: null, preview_uri: null },
  };
}

function requireMockRun(id: string): WorkflowRunDto {
  const run = mockRuns.get(id);
  if (!run) throw new Error("workflow_run.not_found");
  return run;
}

function executeMockRun(runId: string): void {
  const run = requireMockRun(runId);
  if (run.state === "cancelled") return;
  appendMockRunEvent(runId, { type: "run_queued" });
  for (const execution of run.node_executions) {
    appendMockRunEvent(runId, {
      type: "node_started",
      node_execution_id: execution.node_execution_id,
    });
    appendMockRunEvent(runId, {
      type: "node_succeeded",
      node_execution_id: execution.node_execution_id,
      outputs: [],
    });
  }
  const succeeded = { ...run, state: "succeeded" as const, updated_at_epoch_ms: String(Date.now()) };
  mockRuns.set(runId, succeeded);
  appendMockRunEvent(runId, { type: "run_succeeded" });
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

async function getProviders() {
  return [{ id: "mock", name: "Mock", active: true, has_key: false }];
}

async function setActiveProvider(): Promise<void> {}

async function setProviderKey(): Promise<void> {}

async function getAssistantConfig(): Promise<AssistantConfig> {
  return { ...mockAssistantConfig };
}

async function setAssistantConfig(input: AssistantConfigInput): Promise<void> {
  mockAssistantConfig = {
    enabled: input.enabled,
    base_url: input.base_url,
    model: input.model,
    has_key:
      input.clear_api_key
        ? false
        : input.api_key === null
          ? mockAssistantConfig.has_key
          : input.api_key.length > 0,
  };
}

export const mockApi: WorkflowApi = {
  assetsRoot,
  listAssets,
  getAsset,
  listProjects,
  createProject,
  getProject,
  renameProject,
  openProject,
  nodeCapabilityList,
  generationProfileListForCapability,
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
  getProviders,
  setActiveProvider,
  setProviderKey,
  getAssistantConfig,
  setAssistantConfig,
  sendAssistant,
  getPendingAssistantApproval,
  decideAssistantApproval,
};
