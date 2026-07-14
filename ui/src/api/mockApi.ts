// Mock backend — an in-browser stand-in implementing the same WorkflowApi as
// the real Tauri client, so the canvas stays fully interactive with no Rust or
// network (e.g. in a plain `vite dev` browser tab). `selectApi` picks this when
// not running inside a Tauri window.

import type {
  OutputRef,
  RunOutput,
  RunOutputs,
  RunTerminalStatus,
  Workflow,
} from "../workflow/types.ts";
import type {
  AssetDto,
  AssistantConfig,
  AssistantConfigInput,
  CapabilityBundle,
  CapabilityCatalog,
  CapabilityRef,
  CapabilitySearchPage,
  CapabilitySearchRequest,
  RunHandle,
  RunObserver,
  WorkflowApi,
  WorkflowApplyPatchInput,
  WorkflowApplyPatchOutput,
  WorkflowHead,
  WorkflowNodeRef,
  WorkflowPatchBinding,
} from "./types.ts";
import capabilityCatalogFixture from "../__fixtures__/capability_catalog.json";
import { createRunId } from "./runId.ts";
import {
  decideAssistantApproval,
  getPendingAssistantApproval,
  sendAssistant,
} from "./mockAssistant.ts";

const STEP_MS = 400;
const mockHeads = new Map<string, WorkflowHead>();
const MOCK_PROJECT_ID = "mock-project";
const MOCK_PROJECT_NAME = "Mock Project";
let mockAssistantConfig: AssistantConfig = {
  enabled: false,
  base_url: "https://api.openai.com/v1",
  model: "gpt-5.4",
  has_key: false,
};

function outputForNodeType(
  type: string,
  nodeId: string,
): { name: string; output: RunOutput } | null {
  switch (type) {
    case "TextToImage":
      return { name: "image", output: { kind: "image", value: `mock://image/${nodeId}` } };
    case "ImageToVideo":
      return { name: "video", output: { kind: "video", value: `mock://video/${nodeId}` } };
    case "TextToAudio":
      return { name: "audio", output: { kind: "audio", value: `mock://audio/${nodeId}` } };
    case "TextPrompt":
      return { name: "text", output: { kind: "string", value: `mock://text/${nodeId}` } };
    default:
      return null;
  }
}

/**
 * Simulates executing `workflow`, emitting running/succeeded transitions.
 * Nodes run in array order (a stand-in for the engine's topological order).
 * Outputs use the same nested nodeId -> outputName shape as the real backend.
 */
function runWorkflow(workflow: Workflow, observe: RunObserver): RunHandle {
  return new MockWorkflowRun(workflow, observe).handle();
}

class MockWorkflowRun {
  private readonly runId = createRunId();
  private readonly timers: ReturnType<typeof setTimeout>[] = [];
  private readonly outputs: RunOutputs = {};
  private started = false;
  private cancelRequested = false;
  private terminal = false;

  constructor(
    private readonly workflow: Workflow,
    private readonly observe: RunObserver,
  ) {
    this.schedule(() => this.start(), 0);
  }

  handle(): RunHandle {
    return { runId: this.runId, cancel: () => this.cancel() };
  }

  private start(): void {
    this.started = true;
    if (this.cancelRequested) {
      this.finish({ state: "cancelled" });
      return;
    }
    if (this.workflow.nodes.length === 0) {
      this.finish({ state: "succeeded", outputs: this.outputs });
      return;
    }
    this.workflow.nodes.forEach((node, index) => {
      this.schedule(() => this.runNode(node.id, node.type, index), STEP_MS * (index + 1));
    });
  }

  private runNode(nodeId: string, nodeType: string, index: number): void {
    if (this.terminal || this.cancelRequested) return;
    this.observe.onProgress({ nodeId, progress: 0.5, nodeState: "running" });
    const produced = outputForNodeType(nodeType, nodeId);
    if (produced) {
      this.outputs[nodeId] = { [produced.name]: produced.output };
    }
    this.observe.onProgress({ nodeId, progress: 1, nodeState: "done" });
    if (index === this.workflow.nodes.length - 1) {
      this.finish({ state: "succeeded", outputs: this.outputs });
    }
  }

  private cancel(): void {
    if (this.terminal || this.cancelRequested) return;
    this.cancelRequested = true;
    this.observe.onStatus({ state: "cancelling" });
    if (!this.started) return;
    this.clearTimers();
    this.schedule(() => this.finish({ state: "cancelled" }), 0);
  }

  private finish(status: RunTerminalStatus): void {
    if (this.terminal) return;
    this.terminal = true;
    this.clearTimers();
    this.observe.onStatus(status);
  }

  private schedule(callback: () => void, delay: number): void {
    this.timers.push(setTimeout(callback, delay));
  }

  private clearTimers(): void {
    this.timers.forEach(clearTimeout);
  }
}

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
  return [{ id: MOCK_PROJECT_ID, name: MOCK_PROJECT_NAME, created_at: 0 }];
}

async function createProject(name: string) {
  return { id: MOCK_PROJECT_ID, name, created_at: 0 };
}

async function openProject(id: string) {
  return {
    project: { id, name: MOCK_PROJECT_NAME, created_at: 0 },
    workflow_head: mockHeads.get(id) ?? null,
  };
}

async function searchCapabilities(request: CapabilitySearchRequest): Promise<CapabilitySearchPage> {
  const query = request.query.trim().toLowerCase();
  const category = request.category?.trim().toLowerCase() || null;
  const offset = Number(request.cursor ?? "0");
  const limit = Math.min(Math.max(request.limit ?? 24, 1), 24);
  const entries = mockCatalog().capabilities
    .filter((entry) => category === null || entry.presentation.category === category)
    .filter((entry) => {
      if (!query) return true;
      const fields = [
        entry.contract.reference.id,
        entry.presentation.label,
        entry.presentation.description,
        entry.presentation.category,
        ...entry.presentation.search_terms,
      ].map((field) => field.toLowerCase());
      return query.split(/\s+/).every((term) => fields.some((field) => field.includes(term)));
    })
    .map(({ contract, presentation, status }) => ({
      reference: contract.reference,
      presentation,
      status,
    }));
  const page = entries.slice(offset, offset + limit);
  return {
    capabilities: page,
    next_cursor: offset + page.length < entries.length ? String(offset + page.length) : null,
  };
}

async function getCapabilityBundles(refs: CapabilityRef[]): Promise<{ capabilities: CapabilityBundle[] }> {
  const catalog = mockCatalog();
  return {
    capabilities: refs.map((reference) => {
      const entry = catalog.capabilities.find(
        (candidate) =>
          candidate.contract.reference.id === reference.id &&
          candidate.contract.reference.version === reference.version,
      );
      return entry
        ? { reference, contract: entry.contract, presentation: entry.presentation, status: entry.status }
        : {
            reference,
            contract: null,
            presentation: null,
            status: {
              availability: "degraded" as const,
              reason: "exact capability version is unavailable",
              provider_health: null,
              status_revision: 0,
            },
          };
    }),
  };
}

function mockCatalog(): CapabilityCatalog {
  return capabilityCatalogFixture as unknown as CapabilityCatalog;
}

async function applyWorkflowPatch(
  projectId: string,
  _requestId: string,
  input: WorkflowApplyPatchInput,
): Promise<WorkflowApplyPatchOutput> {
  const current = mockHeads.get(projectId) ?? null;
  if ((current?.revision ?? null) !== input.expected_revision) {
    throw new Error("WORKFLOW_REVISION_CONFLICT");
  }
  const workflow = structuredClone(
    current?.workflow ?? { version: "1.0", project_id: projectId, nodes: [] },
  );
  const aliases = new Map<string, string>();
  for (const operation of input.operations) {
    if (operation.op === "add_node") {
      const nodeId = nextMockNodeId(workflow);
      aliases.set(operation.alias, nodeId);
      workflow.nodes.push({
        id: nodeId,
        type: operation.capability.id,
        contract_version: operation.capability.version,
        params: operation.params,
        inputs: {},
        ...(operation.position === null ? {} : { position: operation.position }),
      });
      continue;
    }
    const nodeId = resolveMockNode(operation.node, aliases);
    const node = workflow.nodes.find((candidate) => candidate.id === nodeId);
    if (!node) throw new Error("NODE_NOT_FOUND");
    if (operation.op === "replace_params") node.params = operation.params;
    if (operation.op === "set_position") node.position = operation.position;
    if (operation.op === "clear_input") delete node.inputs[operation.input];
    if (operation.op === "set_input") {
      node.inputs[operation.input] = mockBinding(operation.binding, workflow, aliases);
    }
    if (operation.op === "remove_node") {
      workflow.nodes = workflow.nodes.filter((candidate) => candidate.id !== nodeId);
      removeMockBindings(workflow, nodeId);
    }
  }
  if (input.operations.length === 0 && current === null) {
    return patchOutput(null, aliases, false);
  }
  const next = { project_id: projectId, revision: (current?.revision ?? 0) + 1, workflow };
  mockHeads.set(projectId, next);
  return patchOutput(next, aliases, true);
}

function nextMockNodeId(workflow: Workflow): string {
  let index = 1;
  while (workflow.nodes.some((node) => node.id === `n${index}`)) index += 1;
  return `n${index}`;
}

function resolveMockNode(node: WorkflowNodeRef, aliases: Map<string, string>): string {
  if (node.kind === "id") return node.id;
  const resolved = aliases.get(node.alias);
  if (!resolved) throw new Error("ALIAS_NOT_FOUND");
  return resolved;
}

function mockBinding(
  binding: WorkflowPatchBinding,
  workflow: Workflow,
  aliases: Map<string, string>,
) {
  const outputRef = (source: WorkflowNodeRef) => {
    const nodeId = resolveMockNode(source, aliases);
    const sourceNode = workflow.nodes.find((node) => node.id === nodeId);
    if (!sourceNode) throw new Error("NODE_NOT_FOUND");
    return { node_id: nodeId, output: outputForNodeType(sourceNode.type, nodeId)?.name ?? "output" };
  };
  return binding.kind === "single"
    ? { kind: "single" as const, source: outputRef(binding.source) }
    : { kind: "ordered_many" as const, sources: binding.sources.map(outputRef) };
}

function removeMockBindings(workflow: Workflow, nodeId: string): void {
  for (const node of workflow.nodes) {
    for (const [input, binding] of Object.entries(node.inputs)) {
      const refs: OutputRef[] = Array.isArray(binding)
        ? [binding]
        : "kind" in binding
          ? binding.kind === "ordered_many"
            ? binding.sources
            : [binding.source]
          : [binding];
      const sources = refs.map((source) =>
        Array.isArray(source) ? source[0] : source.node_id,
      );
      if (sources.includes(nodeId)) delete node.inputs[input];
    }
  }
}

function patchOutput(
  workflowHead: WorkflowHead | null,
  aliases: Map<string, string>,
  changed: boolean,
): WorkflowApplyPatchOutput {
  return {
    workflow_head: workflowHead,
    aliases: [...aliases].map(([alias, node_id]) => ({ alias, node_id })),
    readiness_blockers: [],
    changed,
    deduplicated: false,
    undo_id: changed && workflowHead ? `mock-undo-${workflowHead.revision}` : null,
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
  runWorkflow,
  assetsRoot,
  listAssets,
  getAsset,
  listProjects,
  createProject,
  openProject,
  searchCapabilities,
  getCapabilityBundles,
  applyWorkflowPatch,
  getProviders,
  setActiveProvider,
  setProviderKey,
  getAssistantConfig,
  setAssistantConfig,
  sendAssistant,
  getPendingAssistantApproval,
  decideAssistantApproval,
};
