import { useCallback, useEffect, useRef } from "react";
import type { Edge, Node } from "@xyflow/react";
import { api, type RunHandle, type RunObserver } from "../api/index.ts";
import { toWorkflow } from "./serialize.ts";
import type {
  RunLifecycleStatus,
  RunProgress,
  RunStatus,
  RunTerminalStatus,
} from "./types.ts";

interface RunControllerOptions {
  nodes: Node[];
  edges: Edge[];
  projectId: string | null;
  setStatus: (status: RunStatus) => void;
  resetProjection: () => void;
  applyProgress: (progress: RunProgress) => void;
  settleProjection: (status: RunTerminalStatus) => void;
  onSucceeded: () => void;
}

export function useRunController(options: RunControllerOptions) {
  const runHandle = useRef<RunHandle | null>(null);
  const runRequest = useRef(0);

  const stopActiveRun = useCallback(() => {
    runRequest.current += 1;
    const current = runHandle.current;
    runHandle.current = null;
    current?.cancel();
  }, []);

  const invalidateRun = useCallback(() => {
    stopActiveRun();
    options.resetProjection();
    options.setStatus({ state: "idle" });
  }, [options.resetProjection, options.setStatus, stopActiveRun]);

  useEffect(
    () => () => {
      stopActiveRun();
    },
    [stopActiveRun],
  );

  const run = useCallback(() => {
    const workflow = toWorkflow(options.nodes, options.edges, options.projectId ?? "default");
    const request = ++runRequest.current;
    let cancellationPending = false;
    options.resetProjection();
    const observer: RunObserver = {
      onProgress: (progress) => {
        if (request !== runRequest.current) return;
        options.applyProgress(progress);
        if (!cancellationPending) {
          options.setStatus({ state: "running", ...progress });
        }
      },
      onStatus: (status) => {
        if (request !== runRequest.current) return;
        cancellationPending = status.state === "cancelling";
        options.setStatus(status);
        if (!isTerminal(status)) return;
        runHandle.current = null;
        options.settleProjection(status);
        if (status.state === "succeeded") options.onSucceeded();
      },
    };
    options.setStatus({ state: "running", nodeId: workflow.nodes[0]?.id ?? "", progress: 0 });
    runHandle.current = api.runWorkflow(workflow, observer);
  }, [options]);

  const cancel = useCallback(() => runHandle.current?.cancel(), []);

  return { cancel, invalidateRun, run };
}

function isTerminal(status: RunLifecycleStatus): status is RunTerminalStatus {
  return status.state === "succeeded" || status.state === "failed" || status.state === "cancelled";
}
