import { useCallback, useEffect, useRef } from "react";
import { api } from "../api/index.ts";
import type { Workflow } from "./types.ts";

const AUTOSAVE_DELAY_MS = 200;

export function useWorkflowPersistence(
  workflow: Workflow | null,
  onError: (error: unknown) => void,
) {
  const persistedWorkflows = useRef(new Map<string, string>());
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  const workflowRef = useRef({ workflow, revision: 0 });
  const onErrorRef = useRef(onError);
  if (workflowRef.current.workflow !== workflow) {
    workflowRef.current = { workflow, revision: workflowRef.current.revision + 1 };
  }
  onErrorRef.current = onError;

  const markPersisted = useCallback((persisted: Workflow) => {
    persistedWorkflows.current.set(persisted.project_id, JSON.stringify(persisted));
  }, []);

  const persist = useCallback((next: Workflow): Promise<void> => {
    const serialized = JSON.stringify(next);
    const operation = saveQueue.current
      .catch(() => undefined)
      .then(async () => {
        if (persistedWorkflows.current.get(next.project_id) === serialized) {
          return;
        }
        await api.saveWorkflow(next);
        persistedWorkflows.current.set(next.project_id, serialized);
      });
    saveQueue.current = operation;
    return operation;
  }, []);

  const saveCurrent = useCallback(async () => {
    while (true) {
      const current = workflowRef.current;
      if (!current.workflow) {
        return;
      }
      const currentWorkflow = current.workflow;
      try {
        await persist(currentWorkflow);
      } catch (error: unknown) {
        if (workflowRef.current.revision === current.revision) {
          throw error;
        }
        continue;
      }
      const latest = workflowRef.current;
      if (!latest.workflow || latest.revision === current.revision) {
        return;
      }
    }
  }, [persist]);

  useEffect(() => {
    if (!workflow) {
      return;
    }
    const revision = workflowRef.current.revision;
    const timer = window.setTimeout(() => {
      void persist(workflow).catch((error: unknown) => {
        if (workflowRef.current.revision === revision) {
          onError(error);
        }
      });
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [onError, persist, workflow]);

  useEffect(() => {
    const flush = () => {
      void saveCurrent().catch((error: unknown) => onErrorRef.current(error));
    };
    window.addEventListener("pagehide", flush);
    return () => {
      window.removeEventListener("pagehide", flush);
      flush();
    };
  }, [saveCurrent]);

  return { markPersisted, saveCurrent };
}
