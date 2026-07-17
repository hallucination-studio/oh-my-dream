import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Workflow } from "./types.ts";
interface WorkflowPersistenceController {
  enqueueDraft: (workflow: Workflow) => Promise<void>;
  noteDraft: (workflow: Workflow) => number;
  runAfterBarrier: (
    reason: "close",
    action: () => undefined,
  ) => Promise<undefined>;
  hasPendingWork: () => boolean;
  failure: () => unknown;
}

const AUTOSAVE_DELAY_MS = 200;

export function useWorkflowPersistence(
  workflow: Workflow | null,
  controller: WorkflowPersistenceController,
  onError: (error: unknown) => void,
) {
  const persistedWorkflows = useRef(new Map<string, string>());
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  const workflowRef = useRef({ workflow, revision: 0 });
  const onErrorRef = useRef(onError);
  const closingRef = useRef(false);
  const [closeError, setCloseError] = useState<unknown>(null);
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
        await controller.enqueueDraft(next);
        persistedWorkflows.current.set(next.project_id, serialized);
      });
    saveQueue.current = operation;
    return operation;
  }, [controller]);

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

  useLayoutEffect(() => {
    if (!workflow) {
      return;
    }
    controller.noteDraft(workflow);
    const revision = workflowRef.current.revision;
    const timer = window.setTimeout(() => {
      void persist(workflow).catch((error: unknown) => {
        if (workflowRef.current.revision === revision) {
          onError(error);
        }
      });
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [controller, onError, persist, workflow]);

  useEffect(() => {
    const flush = () => {
      void saveCurrent()
        .then(() => controller.runAfterBarrier("close", () => undefined))
        .catch((error: unknown) => onErrorRef.current(error));
    };
    const beforeUnload = (event: BeforeUnloadEvent) => {
      flush();
      const current = workflowRef.current.workflow;
      const dirty = current !== null &&
        persistedWorkflows.current.get(current.project_id) !== JSON.stringify(current);
      if (dirty || controller.hasPendingWork() || controller.failure() !== null) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("pagehide", flush);
    window.addEventListener("beforeunload", beforeUnload);
    return () => {
      window.removeEventListener("pagehide", flush);
      window.removeEventListener("beforeunload", beforeUnload);
      flush();
    };
  }, [controller, saveCurrent]);

  useEffect(() => {
    if (!isTauriWindow()) {
      return;
    }
    let unlisten: (() => void) | undefined;
    const windowHandle = getCurrentWindow();
    void windowHandle
      .onCloseRequested(async (event) => {
        if (closingRef.current) {
          return;
        }
        event.preventDefault();
        try {
          await saveCurrent();
          await controller.runAfterBarrier("close", () => undefined);
          closingRef.current = true;
          await windowHandle.destroy();
        } catch (error: unknown) {
          setCloseError(error);
        }
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch((error: unknown) => onErrorRef.current(error));
    return () => unlisten?.();
  }, [controller, saveCurrent]);

  const keepEditing = useCallback(() => {
    closingRef.current = false;
    setCloseError(null);
  }, []);

  const discardAndClose = useCallback(() => {
    if (!isTauriWindow()) {
      return;
    }
    closingRef.current = true;
    void getCurrentWindow()
      .destroy()
      .catch((error: unknown) => {
        closingRef.current = false;
        setCloseError(error);
      });
  }, []);

  return { closeError, discardAndClose, keepEditing, markPersisted, saveCurrent };
}

function isTauriWindow(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
