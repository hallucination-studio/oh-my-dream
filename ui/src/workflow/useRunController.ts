import { useCallback, useEffect, useRef } from "react";
import {
  api,
  type WorkflowDto,
  type WorkflowRunDto,
  type WorkflowRunEventPageDto,
} from "../api/index.ts";
import type { RunOutputs, RunProgress, RunStatus, RunTerminalStatus } from "./types.ts";
import { failureCopy } from "./failureCopy.ts";

/** Accumulates one node's outputs from a `node_succeeded` payload, mirroring the backend shape. */
function accumulateOutputs(
  target: RunOutputs,
  nodeId: string,
  outputs: unknown,
): void {
  if (!Array.isArray(outputs)) return;
  for (const entry of outputs) {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) continue;
    const { key, value } = entry as { key?: unknown; value?: unknown };
    if (typeof key !== "string" || typeof value !== "object" || value === null) continue;
    const record = value as Record<string, unknown>;
    const type = record.type;
    if (type === "image" || type === "video" || type === "audio") {
      const preview = typeof record.preview_uri === "string" ? record.preview_uri : null;
      const assetId = typeof record.asset_id === "string" ? record.asset_id : "";
      (target[nodeId] ??= {})[key] = { kind: type, value: preview ?? assetId };
    } else if (type === "text") {
      (target[nodeId] ??= {})[key] = {
        kind: "string",
        value: typeof record.value === "string" ? record.value : "",
      };
    }
  }
}

interface RunControllerOptions {
  getWorkflow: () => WorkflowDto | null;
  setStatus: (status: RunStatus) => void;
  resetProjection: () => void;
  applyProgress: (progress: RunProgress) => void;
  settleProjection: (status: RunTerminalStatus) => void;
  onSucceeded: () => void;
  onRunChanged?: (run: WorkflowRunDto | null) => void;
}

export function useRunController(options: RunControllerOptions) {
  const {
    getWorkflow,
    setStatus,
    resetProjection,
    applyProgress,
    settleProjection,
    onSucceeded,
    onRunChanged,
  } = options;
  const generation = useRef(0);
  const activeRun = useRef<WorkflowRunDto | null>(null);
  const runOutputs = useRef<RunOutputs>({});
  const stopListening = useRef<(() => void) | null>(null);
  const seen = useRef(new Set<string>());
  const lastSequence = useRef(0n);
  const eventQueue = useRef(Promise.resolve());

  const invalidateRun = useCallback(() => {
    generation.current += 1;
    activeRun.current = null;
    runOutputs.current = {};
    onRunChanged?.(null);
    stopListening.current?.();
    stopListening.current = null;
    seen.current.clear();
    lastSequence.current = 0n;
    resetProjection();
    setStatus({ state: "idle" });
  }, [onRunChanged, resetProjection, setStatus]);

  useEffect(() => invalidateRun, [invalidateRun]);

  const run = useCallback(async (throughNodeId?: string) => {
    const workflow = getWorkflow();
    if (!workflow) {
      setStatus({ state: "failed", reason: "Open a Project before running a Workflow" });
      return;
    }
    invalidateRun();
    const request = ++generation.current;
    setStatus({ state: "running", nodeId: workflow.nodes[0]?.node_id ?? "", progress: 0 });
    stopListening.current = await api.observeWorkflowRunEvents((event) => {
      eventQueue.current = eventQueue.current.then(() => processEvent(event, request));
    });
    try {
      const admitted = await api.workflowStartRun(
        workflow.project_id,
        workflow.workflow_id,
        workflow.revision,
        throughNodeId
          ? { kind: "through_node", node_id: throughNodeId }
          : { kind: "whole_workflow" },
      );
      if (request !== generation.current) return;
      activeRun.current = admitted;
      onRunChanged?.(admitted);
      await repairEvents(admitted, request);
    } catch (error: unknown) {
      if (request === generation.current) {
        setStatus({ state: "failed", reason: failureCopy("Run workflow", error) });
      }
    }
  }, [getWorkflow, invalidateRun, onRunChanged, setStatus]);

  const cancel = useCallback(() => {
    const run = activeRun.current;
    if (!run) return;
    setStatus({ state: "cancelling" });
    void api.workflowCancelRun(run.project_id, run.workflow_run_id).catch((error: unknown) => {
      setStatus({ state: "cancel_failed", reason: failureCopy("Cancel run", error) });
    });
  }, [setStatus]);

  return { cancel, invalidateRun, run };

  async function processEvent(
    event: import("../api/types.ts").DurableWorkflowRunEventDto,
    request: number,
  ): Promise<void> {
    const run = activeRun.current;
    if (!run || event.workflow_run_id !== run.workflow_run_id || request !== generation.current) {
      return;
    }
    const sequence = BigInt(event.sequence);
    if (sequence > lastSequence.current + 1n) {
      await repairEvents(run, request);
    }
    acceptEvent(event, run, request);
  }

  async function repairEvents(run: WorkflowRunDto, request: number): Promise<void> {
    let cursor = lastSequence.current === 0n ? null : String(lastSequence.current);
    while (request === generation.current) {
      const page: WorkflowRunEventPageDto = await api.workflowListRunEvents(
        run.project_id,
        run.workflow_run_id,
        cursor,
        500,
      );
      for (const event of page.events) acceptEvent(event, run, request);
      if (page.next_sequence === null) return;
      cursor = page.next_sequence;
    }
  }

  function acceptEvent(
    event: import("../api/types.ts").DurableWorkflowRunEventDto,
    run: WorkflowRunDto,
    request: number,
  ): void {
    const key = `${event.workflow_run_id}\0${event.sequence}`;
    if (request !== generation.current || seen.current.has(key)) return;
    seen.current.add(key);
    const sequence = BigInt(event.sequence);
    if (sequence > lastSequence.current) lastSequence.current = sequence;
    const type = event.payload.type;
    const executionId =
      typeof event.payload.node_execution_id === "string"
        ? event.payload.node_execution_id
        : null;
    const execution = run.node_executions.find(
      (candidate) => candidate.node_execution_id === executionId,
    );
    if (execution && type.startsWith("node_")) {
      const terminal = type === "node_succeeded";
      if (terminal) {
        accumulateOutputs(runOutputs.current, execution.node_id, event.payload.outputs);
      }
      const progressBasisPoints =
        typeof event.payload.progress_basis_points === "number"
          ? event.payload.progress_basis_points
          : terminal
            ? 10_000
            : 0;
      const progress: RunProgress = {
        nodeId: execution.node_id,
        progress: progressBasisPoints / 10_000,
        nodeState:
          type === "node_failed" || type === "node_blocked"
            ? "error"
            : terminal
              ? "done"
              : "running",
      };
      applyProgress(progress);
      setStatus({ state: "running", ...progress });
    }
    if (type === "run_succeeded") {
      settle({ state: "succeeded", outputs: runOutputs.current, steps: run.node_executions.length });
    }
    if (type === "run_cancelled") settle({ state: "cancelled" });
    if (type === "run_failed") settle({ state: "failed", reason: "Workflow Run failed" });
  }

  function settle(status: RunTerminalStatus): void {
    onRunChanged?.(activeRun.current);
    activeRun.current = null;
    stopListening.current?.();
    stopListening.current = null;
    setStatus(status);
    settleProjection(status);
    if (status.state === "succeeded") onSucceeded();
  }
}
