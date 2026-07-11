import { useCallback, useEffect, useRef } from "react";
import type { Edge, Node } from "@xyflow/react";
import { api, type RunHandle } from "../api/index.ts";
import { toWorkflow } from "./serialize.ts";
import type { RunStatus } from "./types.ts";

interface RunControllerOptions {
  nodes: Node[];
  edges: Edge[];
  projectId: string | null;
  setStatus: (status: RunStatus) => void;
  resetProjection: () => void;
  applyProjection: (status: RunStatus) => void;
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
    options.resetProjection();
    const observe = (next: RunStatus) => {
      if (request !== runRequest.current) {
        return;
      }
      if (next.state === "succeeded" || next.state === "failed") {
        runHandle.current = null;
      }
      options.setStatus(next);
      options.applyProjection(next);
      if (next.state === "succeeded") {
        options.onSucceeded();
      }
    };
    options.setStatus({ state: "running", nodeId: workflow.nodes[0]?.id ?? "", progress: 0 });
    runHandle.current = api.runWorkflow(workflow, observe);
  }, [options]);

  const cancel = useCallback(() => runHandle.current?.cancel(), []);

  return { cancel, invalidateRun, run };
}
