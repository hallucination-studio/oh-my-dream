import type {
  WorkflowApplyPatchInput,
  WorkflowApplyPatchOutput,
  WorkflowHead,
  WorkflowNodeRef,
  WorkflowPatchOutputRef,
  WorkflowPatchBinding,
  WorkflowPatchOperation,
} from "../api/index.ts";
import type { OutputRef, Workflow, WorkflowInputBinding, WorkflowNode } from "./types.ts";

export type WorkspaceBarrierReason =
  | "assistant_turn"
  | "prepare_run"
  | "undo"
  | "redo"
  | "project_switch"
  | "close";

interface WorkspaceControllerOptions {
  applyPatch: (
    projectId: string,
    requestId: string,
    input: WorkflowApplyPatchInput,
  ) => Promise<WorkflowApplyPatchOutput>;
  projectHead: (head: WorkflowHead) => void;
}

/** Serializes Workflow mutations and owns the UI's authoritative head revision. */
export class WorkspaceController {
  private projectId: string | null = null;
  private head: WorkflowHead | null = null;
  private generation = 0;
  private readonly requestPrefix = createRequestPrefix();
  private requestSequence = 0;
  private pendingCount = 0;
  private draftSequence = 0;
  private draftFingerprint = "";
  private queue: Promise<void> = Promise.resolve();
  private queueFailure: unknown = null;

  constructor(private readonly options: WorkspaceControllerOptions) {}

  activate(projectId: string, head: WorkflowHead | null): void {
    this.projectId = projectId;
    this.head = head;
    this.generation += 1;
    this.queueFailure = null;
    this.draftSequence += 1;
    this.draftFingerprint = JSON.stringify(head?.workflow ?? emptyWorkflow(projectId));
  }

  currentHead(): WorkflowHead | null {
    return this.head;
  }

  failure(): unknown {
    return this.queueFailure;
  }

  hasPendingWork(): boolean {
    return this.pendingCount > 0;
  }

  adoptHead(head: WorkflowHead): void {
    if (head.project_id !== this.projectId || !isNewerHead(head, this.head)) {
      return;
    }
    this.head = head;
    this.options.projectHead(head);
  }

  noteDraft(draft: Workflow): number {
    const fingerprint = JSON.stringify(draft);
    if (fingerprint !== this.draftFingerprint) {
      this.draftFingerprint = fingerprint;
      this.draftSequence += 1;
    }
    return this.draftSequence;
  }

  enqueue(operations: WorkflowPatchOperation[]): Promise<WorkflowApplyPatchOutput> {
    const projectId = this.requireProject();
    const generation = this.generation;
    const requestId = `${this.requestPrefix}-${projectId}-${++this.requestSequence}`;
    const operation = this.queue.then(async () => {
      if (generation !== this.generation || projectId !== this.projectId) {
        throw new Error("Workspace changed before the queued patch could run");
      }
      const input = {
        expected_revision: this.head?.revision ?? null,
        operations,
      };
      try {
        const output = await this.options.applyPatch(projectId, requestId, input);
        this.acceptOutput(projectId, generation, output);
        this.queueFailure = null;
        return output;
      } catch (error: unknown) {
        if (generation === this.generation && projectId === this.projectId) {
          this.queueFailure = error;
        }
        throw error;
      }
    });
    this.track(operation);
    return operation;
  }

  enqueueDraft(draft: Workflow): Promise<void> {
    const projectId = this.requireProject();
    const generation = this.generation;
    const draftSequence = this.noteDraft(draft);
    const operation = this.queue.then(async () => {
      if (generation !== this.generation || projectId !== this.projectId) return;
      const base = this.head?.workflow ?? emptyWorkflow(projectId);
      const operations = workflowPatchOperations(base, draft);
      if (operations.length === 0) return;
      await this.applyQueued(projectId, generation, draftSequence, operations);
    });
    this.track(operation);
    return operation;
  }

  async flush(): Promise<void> {
    await this.queue;
    if (this.queueFailure !== null) {
      throw this.queueFailure;
    }
  }

  async runAfterBarrier<T>(
    _reason: WorkspaceBarrierReason,
    action: () => T | Promise<T>,
    restoreFocus?: () => void,
  ): Promise<T> {
    try {
      await this.flush();
    } catch (error: unknown) {
      restoreFocus?.();
      throw error;
    }
    return action();
  }

  /** Runs a future undo action only after all queued Workflow writes settle. */
  runUndo<T>(action: () => T | Promise<T>, restoreFocus?: () => void): Promise<T> {
    return this.runAfterBarrier("undo", action, restoreFocus);
  }

  /** Runs a future redo action only after all queued Workflow writes settle. */
  runRedo<T>(action: () => T | Promise<T>, restoreFocus?: () => void): Promise<T> {
    return this.runAfterBarrier("redo", action, restoreFocus);
  }

  private acceptOutput(
    projectId: string,
    generation: number,
    output: WorkflowApplyPatchOutput,
    draftSequence?: number,
  ): void {
    if (generation !== this.generation || projectId !== this.projectId) {
      return;
    }
    const next = output.workflow_head;
    if (next === null || next.project_id !== projectId || !isNewerHead(next, this.head)) {
      return;
    }
    this.head = next;
    if (draftSequence !== undefined && draftSequence < this.draftSequence) {
      return;
    }
    this.options.projectHead(next);
  }

  private async applyQueued(
    projectId: string,
    generation: number,
    draftSequence: number,
    operations: WorkflowPatchOperation[],
  ): Promise<void> {
    const requestId = `${this.requestPrefix}-${projectId}-${++this.requestSequence}`;
    try {
      const output = await this.options.applyPatch(projectId, requestId, {
        expected_revision: this.head?.revision ?? null,
        operations,
      });
      this.acceptOutput(projectId, generation, output, draftSequence);
      this.queueFailure = null;
    } catch (error: unknown) {
      if (generation === this.generation && projectId === this.projectId) {
        this.queueFailure = error;
      }
      throw error;
    }
  }

  private track(operation: Promise<unknown>): void {
    this.pendingCount += 1;
    this.queue = operation.then(
      () => {
        this.pendingCount -= 1;
      },
      () => {
        this.pendingCount -= 1;
      },
    );
  }

  private requireProject(): string {
    if (this.projectId === null) {
      throw new Error("Open a project before editing its Workflow");
    }
    return this.projectId;
  }
}

function isNewerHead(candidate: WorkflowHead, current: WorkflowHead | null): boolean {
  return current === null || candidate.revision >= current.revision;
}

export function workflowPatchOperations(
  base: Workflow,
  draft: Workflow,
): WorkflowPatchOperation[] {
  const baseNodes = new Map(base.nodes.map((node) => [node.id, node]));
  const draftNodes = new Map(draft.nodes.map((node) => [node.id, node]));
  const recreated = new Set<string>();
  const operations: WorkflowPatchOperation[] = [];
  for (const node of base.nodes) {
    const next = draftNodes.get(node.id);
    if (!next || !sameCapability(node, next)) {
      operations.push({ op: "remove_node", node: idRef(node.id) });
      if (next) recreated.add(node.id);
    }
  }
  for (const node of draft.nodes) {
    if (!baseNodes.has(node.id) || recreated.has(node.id)) {
      operations.push({
        op: "add_node",
        alias: aliasFor(node.id),
        capability: { id: node.type, version: node.contract_version ?? "1.0" },
        params: node.params,
        position: node.position ?? null,
      });
    }
  }
  for (const node of draft.nodes) {
    const previous = baseNodes.get(node.id);
    if (previous && !recreated.has(node.id)) {
      if (!sameJson(previous.params, node.params)) {
        operations.push({ op: "replace_params", node: idRef(node.id), params: node.params });
      }
      if (node.position && !sameJson(previous.position, node.position)) {
        operations.push({ op: "set_position", node: idRef(node.id), position: node.position });
      }
    }
  }
  appendInputOperations(operations, baseNodes, draft.nodes, recreated);
  return operations;
}

function appendInputOperations(
  operations: WorkflowPatchOperation[],
  baseNodes: Map<string, WorkflowNode>,
  draftNodes: WorkflowNode[],
  recreated: Set<string>,
): void {
  for (const node of draftNodes) {
    const previous = baseNodes.get(node.id);
    const nodeRef = draftRef(node.id, baseNodes, recreated);
    if (previous && !recreated.has(node.id)) {
      for (const input of Object.keys(previous.inputs)) {
        if (!Object.hasOwn(node.inputs, input)) {
          operations.push({ op: "clear_input", node: nodeRef, input });
        }
      }
    }
    for (const [input, binding] of Object.entries(node.inputs)) {
      if (
        !previous ||
        recreated.has(node.id) ||
        bindingReferences(binding, recreated) ||
        !sameBinding(previous.inputs[input], binding)
      ) {
        operations.push({
          op: "set_input",
          node: nodeRef,
          input,
          binding: patchBinding(binding, baseNodes, recreated),
        });
      }
    }
  }
}

function patchBinding(
  binding: WorkflowInputBinding,
  baseNodes: Map<string, WorkflowNode>,
  recreated: Set<string>,
): WorkflowPatchBinding {
  if (Array.isArray(binding)) {
    return { kind: "single", source: patchOutputRef(binding, baseNodes, recreated) };
  }
  if (!("kind" in binding)) {
    return { kind: "single", source: patchOutputRef(binding, baseNodes, recreated) };
  }
  if (binding.kind === "single") {
    return { kind: "single", source: patchOutputRef(binding.source, baseNodes, recreated) };
  }
  return {
    kind: "ordered_many",
    sources: binding.sources.map((source) => patchOutputRef(source, baseNodes, recreated)),
  };
}

function patchOutputRef(
  reference: OutputRef,
  baseNodes: Map<string, WorkflowNode>,
  recreated: Set<string>,
): WorkflowPatchOutputRef {
  return {
    node: draftRef(nodeIdOf(reference), baseNodes, recreated),
    output: outputNameOf(reference),
  };
}

function draftRef(
  nodeId: string,
  baseNodes: Map<string, WorkflowNode>,
  recreated: Set<string>,
): WorkflowNodeRef {
  return !baseNodes.has(nodeId) || recreated.has(nodeId)
    ? { kind: "alias", alias: aliasFor(nodeId) }
    : idRef(nodeId);
}

function nodeIdOf(reference: OutputRef): string {
  return Array.isArray(reference) ? reference[0] : reference.node_id;
}

function outputNameOf(reference: OutputRef): string {
  return Array.isArray(reference) ? reference[1] : reference.output;
}

function idRef(id: string): WorkflowNodeRef {
  return { kind: "id", id };
}

function aliasFor(id: string): string {
  return `draft-${id}`;
}

function sameCapability(left: WorkflowNode, right: WorkflowNode): boolean {
  return left.type === right.type &&
    (left.contract_version ?? "1.0") === (right.contract_version ?? "1.0");
}

function sameJson(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sameBinding(
  left: WorkflowInputBinding | undefined,
  right: WorkflowInputBinding,
): boolean {
  if (!left) return false;
  return sameJson(bindingSources(left), bindingSources(right));
}

function bindingReferences(binding: WorkflowInputBinding, nodeIds: Set<string>): boolean {
  return bindingNodeIds(binding).some((nodeId) => nodeIds.has(nodeId));
}

function bindingNodeIds(binding: WorkflowInputBinding): string[] {
  if (Array.isArray(binding)) return [binding[0]];
  if (!("kind" in binding)) return [binding.node_id];
  const references = binding.kind === "single" ? [binding.source] : binding.sources;
  return references.map(nodeIdOf);
}

function bindingSources(binding: WorkflowInputBinding): { many: boolean; sources: string[] } {
  if (Array.isArray(binding)) return { many: false, sources: [binding.join("\u0000")] };
  if (!("kind" in binding)) {
    return { many: false, sources: [`${binding.node_id}\u0000${binding.output}`] };
  }
  const references = binding.kind === "single" ? [binding.source] : binding.sources;
  return {
    many: binding.kind === "ordered_many",
    sources: references.map((reference) =>
      Array.isArray(reference)
        ? reference.join("\u0000")
        : `${reference.node_id}\u0000${reference.output}`,
    ),
  };
}

function emptyWorkflow(projectId: string): Workflow {
  return { version: "1.0", project_id: projectId, nodes: [] };
}

function createRequestPrefix(): string {
  return `ui-${globalThis.crypto.randomUUID()}`;
}
