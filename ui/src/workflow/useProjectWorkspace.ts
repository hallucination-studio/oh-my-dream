import { useCallback, useEffect, useMemo, useRef } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Edge, Node } from "@xyflow/react";
import { api, type Project, type ProjectWorkspace } from "../api/index.ts";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { fromWorkflow } from "./editor.ts";
import { toWorkflow } from "./serialize.ts";
import type { RunStatus } from "./types.ts";
import { useWorkflowPersistence } from "./useWorkflowPersistence.ts";

interface ProjectWorkspaceOptions {
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
}

export function useProjectWorkspace(options: ProjectWorkspaceOptions) {
  const {
    project,
    nodes,
    edges,
    setNodes,
    setStatus,
    invalidateRun,
  } = options;
  const requestRef = useRef(0);
  const projectRef = useRef(project);
  const unassignedDraft = useRef(false);
  projectRef.current = project;

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
  const persistence = useWorkflowPersistence(activeWorkflow, onPersistenceError);
  const hydrate = useHydrateWorkspace(options, projectRef, unassignedDraft, setParam, persistence.markPersisted);

  useInitialWorkspace(requestRef, unassignedDraft, hydrate, setStatus);
  const openProject = useOpenProject(
    requestRef,
    projectRef,
    unassignedDraft,
    hydrate,
    persistence.saveCurrent,
    options,
  );

  return { markWorkflowMutation, openProject, setParam };
}

function useHydrateWorkspace(
  options: ProjectWorkspaceOptions,
  projectRef: { current: Project | null },
  unassignedDraft: { current: boolean },
  setParam: (nodeId: string, name: string, value: unknown) => void,
  markPersisted: (workflow: ReturnType<typeof toWorkflow>) => void,
) {
  const {
    invalidateRun,
    setProject,
    setNodes,
    setEdges,
    setSelectedId,
    setStatus,
  } = options;
  return useCallback(
    (workspace: ProjectWorkspace, preserveDraft = false) => {
      const graph = fromWorkflow(workspace.workflow_json, setParam);
      const normalized = toWorkflow(graph.nodes, graph.edges, workspace.project.id);
      invalidateRun();
      markPersisted(normalized);
      projectRef.current = workspace.project;
      setProject(workspace.project);
      if (!preserveDraft) {
        setNodes(graph.nodes);
        setEdges(graph.edges);
        setSelectedId(null);
      }
      unassignedDraft.current = false;
      setStatus({ state: "idle" });
    },
    [
      invalidateRun,
      markPersisted,
      projectRef,
      setEdges,
      setNodes,
      setParam,
      setProject,
      setSelectedId,
      setStatus,
      unassignedDraft,
    ],
  );
}

function useInitialWorkspace(
  requestRef: { current: number },
  unassignedDraft: { current: boolean },
  hydrate: (workspace: ProjectWorkspace, preserveDraft?: boolean) => void,
  setStatus: Dispatch<SetStateAction<RunStatus>>,
) {
  useEffect(() => {
    const request = ++requestRef.current;
    let cancelled = false;
    void api
      .listProjects()
      .then((projects) => (projects[0] ? api.openProject(projects[0].id) : null))
      .then((workspace) => {
        if (!cancelled && request === requestRef.current && workspace) {
          hydrate(workspace, unassignedDraft.current);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled && request === requestRef.current) {
          setStatus({ state: "failed", reason: String(error) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [hydrate, requestRef, setStatus, unassignedDraft]);
}

function useOpenProject(
  requestRef: { current: number },
  projectRef: { current: Project | null },
  unassignedDraft: { current: boolean },
  hydrate: (workspace: ProjectWorkspace, preserveDraft?: boolean) => void,
  saveCurrent: () => Promise<void>,
  options: ProjectWorkspaceOptions,
) {
  const { invalidateRun, setProjectsOpen, setStatus } = options;
  return useCallback(
    (id: string) => {
      setProjectsOpen(false);
      const request = ++requestRef.current;
      if (id === projectRef.current?.id) {
        void saveCurrent().catch((error: unknown) => {
          if (request === requestRef.current) {
            setStatus({ state: "failed", reason: String(error) });
          }
        });
        return;
      }
      invalidateRun();
      void saveCurrent()
        .then(() => api.openProject(id))
        .then(async (workspace) => {
          await saveCurrent();
          if (request === requestRef.current) {
            hydrate(workspace, !projectRef.current && unassignedDraft.current);
          }
        })
        .catch((error: unknown) => {
          if (request === requestRef.current) {
            setStatus({ state: "failed", reason: String(error) });
          }
        });
    },
    [
      hydrate,
      invalidateRun,
      projectRef,
      requestRef,
      saveCurrent,
      setProjectsOpen,
      setStatus,
      unassignedDraft,
    ],
  );
}
