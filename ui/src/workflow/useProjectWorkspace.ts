import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Edge, Node } from "@xyflow/react";
import {
  api,
  type Project,
  type ProjectWorkspace,
  type WorkflowHead,
} from "../api/index.ts";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { fromWorkflow } from "./editor.ts";
import { CapabilityContractCache } from "./contractCache.ts";
import { toWorkflow } from "./serialize.ts";
import type { RunStatus } from "./types.ts";
import { useWorkflowPersistence } from "./useWorkflowPersistence.ts";
import {
  WorkspaceController,
  type WorkspaceBarrierReason,
} from "./workspaceController.ts";

export type ProjectWorkspaceState =
  | { state: "booting" }
  | { state: "no_project" }
  | { state: "opening"; project: Project | null; workflowHead: WorkflowHead | null }
  | { state: "ready"; project: Project; workflowHead: WorkflowHead | null }
  | {
      state: "blocked";
      project: Project | null;
      workflowHead: WorkflowHead | null;
      reason: string;
    };

export interface ProjectWorkspaceOptions {
  project: Project | null;
  setProject: Dispatch<SetStateAction<Project | null>>;
  nodes: Node[];
  edges: Edge[];
  setNodes: Dispatch<SetStateAction<Node[]>>;
  setEdges: Dispatch<SetStateAction<Edge[]>>;
  setSelectedId: Dispatch<SetStateAction<string | null>>;
  setProjectsOpen: Dispatch<SetStateAction<boolean>>;
  setStatus: Dispatch<SetStateAction<RunStatus>>;
  invalidateRun: () => void;
  capabilityCache: CapabilityContractCache;
}

export function useProjectWorkspace(options: ProjectWorkspaceOptions) {
  const { project, nodes, edges, setNodes, setStatus, invalidateRun, capabilityCache } = options;
  const [workspaceState, setWorkspaceState] = useState<ProjectWorkspaceState>({
    state: "booting",
  });
  const requestRef = useRef(0);
  const projectRef = useRef(project);
  const workspaceStateRef = useRef(workspaceState);
  const unassignedDraft = useRef(false);
  const projectHeadRef = useRef<(head: WorkflowHead) => void>(() => undefined);
  const markPersistedRef = useRef<(workflow: ReturnType<typeof toWorkflow>) => void>(
    () => undefined,
  );
  const controllerRef = useRef<WorkspaceController | null>(null);
  if (controllerRef.current === null) {
    controllerRef.current = new WorkspaceController({
      applyPatch: api.applyWorkflowPatch,
      projectHead: (head) => projectHeadRef.current(head),
    });
  }
  const controller = controllerRef.current;
  projectRef.current = project;
  workspaceStateRef.current = workspaceState;

  const adoptWorkflowHead = useCallback(
    async (head: WorkflowHead) => {
      await capabilityCache.loadProject(
        head.workflow.nodes.map((node) => ({
          id: node.type,
          version: node.contract_version ?? "1.0",
        })),
      );
      controller.adoptHead(head);
    },
    [capabilityCache, controller],
  );

  const markWorkflowMutation = useCallback(() => {
    if (!projectRef.current) {
      unassignedDraft.current = true;
    }
    invalidateRun();
  }, [invalidateRun]);

  const setParam = useCallback(
    (nodeId: string, name: string, value: unknown) => {
      markWorkflowMutation();
      setNodes((current) =>
        current.map((node) =>
          node.id === nodeId
            ? {
                ...node,
                data: {
                  ...node.data,
                  params: { ...(node.data as FlowNodeData).params, [name]: value },
                },
              }
            : node,
        ),
      );
    },
    [markWorkflowMutation, setNodes],
  );

  const activeWorkflow = useMemo(
    () => (project ? toWorkflow(nodes, edges, project.id) : null),
    [edges, nodes, project],
  );
  const onPersistenceError = useCallback(
    (error: unknown) => setStatus({ state: "failed", reason: String(error) }),
    [setStatus],
  );
  const persistence = useWorkflowPersistence(activeWorkflow, controller, onPersistenceError);
  markPersistedRef.current = persistence.markPersisted;
  projectHeadRef.current = (head) => {
    const graph = fromWorkflow(head.workflow, setParam, capabilityCache.snapshot());
    const normalized = toWorkflow(graph.nodes, graph.edges, head.project_id);
    markPersistedRef.current(normalized);
    setNodes(graph.nodes);
    options.setEdges(graph.edges);
    options.setSelectedId((selected) =>
      selected && graph.nodes.some((node) => node.id === selected) ? selected : null,
    );
    setWorkspaceState((current) => {
      const currentProject = projectOf(current);
      return currentProject?.id === head.project_id
        ? { state: "ready", project: currentProject, workflowHead: head }
        : current;
    });
  };
  const hydrate = useHydrateWorkspace(
    options,
    projectRef,
    unassignedDraft,
    setParam,
    controller,
    persistence.markPersisted,
    setWorkspaceState,
    capabilityCache,
  );

  useInitialWorkspace(requestRef, setWorkspaceState, setStatus);
  const openProject = useOpenProject(
    requestRef,
    projectRef,
    workspaceStateRef,
    unassignedDraft,
    hydrate,
    runAfterBarrier,
    setWorkspaceState,
    options,
  );

  return {
    canEdit: workspaceState.state !== "booting" && workspaceState.state !== "opening" && workspaceState.state !== "blocked",
    markWorkflowMutation,
    openProject,
    runAfterBarrier,
    runUndo: <T>(action: () => T | Promise<T>, restoreFocus?: () => void) =>
      runAfterBarrier("undo", action, restoreFocus),
    runRedo: <T>(action: () => T | Promise<T>, restoreFocus?: () => void) =>
      runAfterBarrier("redo", action, restoreFocus),
    adoptWorkflowHead,
    closeError: persistence.closeError,
    discardAndClose: persistence.discardAndClose,
    keepEditing: persistence.keepEditing,
    setParam,
    workspaceState,
  };

  async function runAfterBarrier<T>(
    reason: WorkspaceBarrierReason,
    action: () => T | Promise<T>,
    restoreFocus?: () => void,
  ): Promise<T> {
    try {
      await persistence.saveCurrent();
    } catch (error: unknown) {
      restoreFocus?.();
      throw error;
    }
    return controller.runAfterBarrier(reason, action, restoreFocus);
  }
}

function useHydrateWorkspace(
  options: ProjectWorkspaceOptions,
  projectRef: { current: Project | null },
  unassignedDraft: { current: boolean },
  setParam: (nodeId: string, name: string, value: unknown) => void,
  controller: WorkspaceController,
  markPersisted: (workflow: ReturnType<typeof toWorkflow>) => void,
  setWorkspaceState: Dispatch<SetStateAction<ProjectWorkspaceState>>,
  capabilityCache: CapabilityContractCache,
) {
  const { invalidateRun, setProject, setNodes, setEdges, setSelectedId, setStatus } = options;
  return useCallback(
    async (workspace: ProjectWorkspace, preserveDraft = false) => {
      const source =
        workspace.workflow_head?.workflow ?? emptyWorkflow(workspace.project.id);
      await capabilityCache.loadProject(
        source.nodes.map((node) => ({ id: node.type, version: node.contract_version ?? "1.0" })),
      );
      const graph = fromWorkflow(source, setParam, capabilityCache.snapshot());
      const normalized = toWorkflow(graph.nodes, graph.edges, workspace.project.id);
      invalidateRun();
      controller.activate(workspace.project.id, workspace.workflow_head);
      markPersisted(normalized);
      projectRef.current = workspace.project;
      setProject(workspace.project);
      if (!preserveDraft) {
        setNodes(graph.nodes);
        setEdges(graph.edges);
        setSelectedId(null);
      }
      unassignedDraft.current = false;
      setWorkspaceState({
        state: "ready",
        project: workspace.project,
        workflowHead: workspace.workflow_head,
      });
      setStatus({ state: "idle" });
    },
    [
      invalidateRun,
      controller,
      markPersisted,
      projectRef,
      setEdges,
      setNodes,
      setParam,
      setProject,
      setSelectedId,
      setStatus,
      setWorkspaceState,
      unassignedDraft,
      capabilityCache,
    ],
  );
}

function useInitialWorkspace(
  requestRef: { current: number },
  setWorkspaceState: Dispatch<SetStateAction<ProjectWorkspaceState>>,
  setStatus: Dispatch<SetStateAction<RunStatus>>,
) {
  useEffect(() => {
    const request = ++requestRef.current;
    let cancelled = false;
    void api
      .listProjects()
      .then(() => {
        if (!cancelled && request === requestRef.current) {
          setWorkspaceState({ state: "no_project" });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled && request === requestRef.current) {
          const reason = String(error);
          setWorkspaceState({ state: "blocked", project: null, workflowHead: null, reason });
          setStatus({ state: "failed", reason });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [requestRef, setStatus, setWorkspaceState]);
}

function useOpenProject(
  requestRef: { current: number },
  projectRef: { current: Project | null },
  workspaceStateRef: { current: ProjectWorkspaceState },
  unassignedDraft: { current: boolean },
  hydrate: (workspace: ProjectWorkspace, preserveDraft?: boolean) => Promise<void>,
  runAfterBarrier: <T>(
    reason: WorkspaceBarrierReason,
    action: () => T | Promise<T>,
    restoreFocus?: () => void,
  ) => Promise<T>,
  setWorkspaceState: Dispatch<SetStateAction<ProjectWorkspaceState>>,
  options: ProjectWorkspaceOptions,
) {
  const { invalidateRun, setProjectsOpen, setStatus } = options;
  return useCallback(
    (id: string) => {
      setProjectsOpen(false);
      const request = ++requestRef.current;
      const previousProject = projectRef.current;
      const previousHead = workflowHeadOf(workspaceStateRef.current);
      if (id === previousProject?.id && workspaceStateRef.current.state === "ready") {
        void runAfterBarrier("project_switch", () => undefined).catch((error: unknown) => {
          if (request === requestRef.current) {
            preserveWorkspaceAfterFlushFailure(
              error,
              previousProject,
              previousHead,
              setWorkspaceState,
              setStatus,
            );
          }
        });
        return;
      }
      setWorkspaceState({ state: "opening", project: previousProject, workflowHead: previousHead });
      invalidateRun();
      let openStarted = false;
      void runAfterBarrier("project_switch", async () => {
        openStarted = true;
        return api.openProject(id);
      })
        .then(async (workspace) => {
          await runAfterBarrier("project_switch", () => undefined);
          if (request === requestRef.current) {
            await hydrate(workspace, !previousProject && unassignedDraft.current);
          }
        })
        .catch((error: unknown) => {
          if (request === requestRef.current) {
            if (openStarted) {
              blockWorkspace(error, previousProject, previousHead, setWorkspaceState, setStatus);
            } else {
              preserveWorkspaceAfterFlushFailure(
                error,
                previousProject,
                previousHead,
                setWorkspaceState,
                setStatus,
              );
            }
          }
        });
    },
    [
      hydrate,
      invalidateRun,
      projectRef,
      requestRef,
      runAfterBarrier,
      setProjectsOpen,
      setStatus,
      setWorkspaceState,
      unassignedDraft,
      workspaceStateRef,
    ],
  );
}

function preserveWorkspaceAfterFlushFailure(
  error: unknown,
  project: Project | null,
  workflowHead: WorkflowHead | null,
  setWorkspaceState: Dispatch<SetStateAction<ProjectWorkspaceState>>,
  setStatus: Dispatch<SetStateAction<RunStatus>>,
) {
  if (project) {
    setWorkspaceState({ state: "ready", project, workflowHead });
  } else {
    setWorkspaceState({ state: "no_project" });
  }
  setStatus({ state: "failed", reason: String(error) });
}

function emptyWorkflow(projectId: string) {
  return { version: "1.0", project_id: projectId, nodes: [] };
}

function workflowHeadOf(state: ProjectWorkspaceState): WorkflowHead | null {
  return state.state === "opening" || state.state === "ready" || state.state === "blocked"
    ? state.workflowHead
    : null;
}

function projectOf(state: ProjectWorkspaceState): Project | null {
  return state.state === "opening" || state.state === "ready" || state.state === "blocked"
    ? state.project
    : null;
}

function blockWorkspace(
  error: unknown,
  project: Project | null,
  workflowHead: WorkflowHead | null,
  setWorkspaceState: Dispatch<SetStateAction<ProjectWorkspaceState>>,
  setStatus: Dispatch<SetStateAction<RunStatus>>,
) {
  const reason = String(error);
  setWorkspaceState({ state: "blocked", project, workflowHead, reason });
  setStatus({ state: "failed", reason });
}
