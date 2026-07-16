import type {
  NodeCapabilityContractDto,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowWithReadinessDto,
} from "../api/types.ts";
import type { Workflow } from "./types.ts";
import { editorMutationActions } from "./canonical.ts";

export type WorkspaceBarrierReason =
  | "assistant_turn"
  | "prepare_run"
  | "undo"
  | "redo"
  | "project_switch"
  | "close";

interface Options {
  applyMutation: (
    projectId: string,
    workflowId: string,
    baseRevision: string,
    actions: WorkflowMutationActionDto[],
  ) => Promise<WorkflowWithReadinessDto>;
  workflowChanged: (workflow: WorkflowDto) => void;
  contracts: () => readonly NodeCapabilityContractDto[];
}

/** Serializes canonical Workflow mutations and owns the accepted revision. */
export class CanonicalWorkspaceController {
  private projectId: string | null = null;
  private workflow: WorkflowDto | null = null;
  private generation = 0;
  private pendingCount = 0;
  private draftSequence = 0;
  private draftFingerprint = "";
  private queue: Promise<void> = Promise.resolve();
  private queueFailure: unknown = null;

  constructor(private readonly options: Options) {}

  activate(projectId: string, workflow: WorkflowDto): void {
    this.projectId = projectId;
    this.workflow = workflow;
    this.generation += 1;
    this.queueFailure = null;
    this.draftSequence += 1;
    this.draftFingerprint = "";
  }

  failure(): unknown {
    return this.queueFailure;
  }

  hasPendingWork(): boolean {
    return this.pendingCount > 0;
  }

  noteDraft(draft: Workflow): number {
    const fingerprint = JSON.stringify(draft);
    if (fingerprint !== this.draftFingerprint) {
      this.draftFingerprint = fingerprint;
      this.draftSequence += 1;
    }
    return this.draftSequence;
  }

  enqueueDraft(draft: Workflow): Promise<void> {
    const sequence = this.noteDraft(draft);
    const projectId = this.requireProject();
    const generation = this.generation;
    const operation = this.queue.then(async () => {
      const base = this.requireWorkflow();
      if (generation !== this.generation || projectId !== this.projectId) return;
      const actions = editorMutationActions(base, draft, this.options.contracts());
      if (actions.length === 0) return;
      const result = await this.apply(projectId, generation, base, actions, false);
      if (sequence === this.draftSequence) this.options.workflowChanged(result.workflow);
    });
    this.track(operation);
    return operation;
  }

  async flush(): Promise<void> {
    await this.queue;
    if (this.queueFailure !== null) throw this.queueFailure;
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

  private async apply(
    projectId: string,
    generation: number,
    base: WorkflowDto,
    actions: WorkflowMutationActionDto[],
    publish = true,
  ): Promise<WorkflowWithReadinessDto> {
    try {
      const result = await this.options.applyMutation(
        projectId,
        base.workflow_id,
        base.revision,
        actions,
      );
      if (generation === this.generation && projectId === this.projectId) {
        this.workflow = result.workflow;
        this.queueFailure = null;
        if (publish) this.options.workflowChanged(result.workflow);
      }
      return result;
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
    if (this.projectId === null) throw new Error("Open a Project before editing its Workflow");
    return this.projectId;
  }

  private requireWorkflow(): WorkflowDto {
    if (this.workflow === null) throw new Error("The Project Workflow is not loaded");
    return this.workflow;
  }
}
