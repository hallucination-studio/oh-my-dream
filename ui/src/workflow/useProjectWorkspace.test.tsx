import { act, renderHook, waitFor } from "@testing-library/react";
import type { Edge, Node } from "@xyflow/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { api } from "../api/index.ts";
import type { Project } from "../api/types.ts";
import { useProjectWorkspace, type ProjectWorkspaceOptions } from "./useProjectWorkspace.ts";
import type { RunStatus } from "./types.ts";

const PROJECT_ID = "10000000-0000-4000-8000-000000000001";
const WORKFLOW_ID = "20000000-0000-4000-8000-000000000001";
const NODE_ID = "30000000-0000-4000-8000-000000000001";

afterEach(() => vi.restoreAllMocks());

describe("useProjectWorkspace canonical activation", () => {
  it("loads the current canonical Workflow after opening a Project", async () => {
    mockCanonicalBackend();
    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));

    act(() => view.result.current.openProject(PROJECT_ID));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    expect(view.result.current.nodes).toHaveLength(1);
    expect(view.result.current.nodes[0]?.id).toBe(NODE_ID);
    expect(view.result.current.nodes[0]?.data.params).toEqual({ text: "hello" });
  });

  it("persists an edited parameter through workflow_apply_mutation", async () => {
    mockCanonicalBackend();
    const apply = vi.spyOn(api, "workflowApplyMutation").mockImplementation(
      async (_projectId, _workflowId, _revision, actions) => ({
        workflow: {
          ...workflow(),
          revision: "2",
          nodes: [{
            ...workflow().nodes[0]!,
            parameters: actions[0]?.kind === "replace_node_parameters"
              ? actions[0].parameters
              : workflow().nodes[0]!.parameters,
          }],
        },
        readiness: { state: "ready" },
      }),
    );
    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));
    act(() => view.result.current.openProject(PROJECT_ID));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    act(() => view.result.current.setParam(NODE_ID, "text", "edited"));
    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(apply.mock.calls[0]?.[3]).toEqual([{
      kind: "replace_node_parameters",
      node_id: NODE_ID,
      parameters: [{ key: "text", value: { kind: "text", value: "edited" } }],
    }]);
  });

  it("ignores a stale Project-open response after a newer selection wins", async () => {
    const betaProjectId = "10000000-0000-4000-8000-000000000002";
    const betaWorkflowId = "20000000-0000-4000-8000-000000000002";
    const beta = deferred<Awaited<ReturnType<typeof api.openProject>>>();
    mockCanonicalBackend();
    vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === betaProjectId
        ? beta.promise
        : Promise.resolve(workspaceSummary(testProject(), WORKFLOW_ID))
    );
    vi.spyOn(api, "workflowGetCurrent").mockImplementation(async (id) => ({
      workflow: id === betaProjectId
        ? { ...workflow(), project_id: betaProjectId, workflow_id: betaWorkflowId }
        : workflow(),
      readiness: { state: "ready" },
    }));
    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));

    act(() => view.result.current.openProject(betaProjectId));
    act(() => view.result.current.openProject(PROJECT_ID));
    await waitFor(() => expect(view.result.current.workspaceState).toMatchObject({
      state: "ready",
      project: { id: PROJECT_ID },
    }));
    await act(async () => {
      beta.resolve(workspaceSummary(
        { ...testProject(), id: betaProjectId, name: "Beta" },
        betaWorkflowId,
      ));
      await beta.promise;
    });
    expect(view.result.current.workspaceState).toMatchObject({
      state: "ready",
      project: { id: PROJECT_ID },
    });
  });
});

function mockCanonicalBackend(): void {
  const project = testProject();
  vi.spyOn(api, "listProjects").mockResolvedValue([project]);
  vi.spyOn(api, "openProject").mockResolvedValue({
    project,
    current_workflow_summary: {
      workflow_id: WORKFLOW_ID,
      workflow_revision: "1",
      readiness: "ready",
    },
  });
  vi.spyOn(api, "workflowGetCurrent").mockResolvedValue({
    workflow: workflow(),
    readiness: { state: "ready" },
  });
  vi.spyOn(api, "nodeCapabilityList").mockResolvedValue([{
    capability_ref: { id: "text.provide_literal", version: "1.0" },
    parameters: [{
      key: "text",
      constraint: { kind: "text_utf8_bytes", minimum: 1, maximum: 65536 },
      presence: { kind: "required" },
    }],
    inputs: [],
    outputs: [{ key: "text", data_type: "text", is_primary: true }],
    execution_kind: "pure_value",
  }]);
}

function workflow() {
  return {
    schema_version: 1,
    workflow_id: WORKFLOW_ID,
    project_id: PROJECT_ID,
    revision: "1",
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "1",
    nodes: [{
      node_id: NODE_ID,
      capability_id: "text.provide_literal",
      capability_version: "1.0",
      parameters: [{ key: "text", value: { kind: "text" as const, value: "hello" } }],
      canvas_position: { x: 100, y: 100 },
    }],
    input_bindings: [],
  };
}

function useWorkspaceHarness() {
  const [project, setProject] = useState<Project | null>(null);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [, setSelectedId] = useState<string | null>(null);
  const [, setProjectsOpen] = useState(false);
  const [, setStatus] = useState<RunStatus>({ state: "idle" });
  const options: ProjectWorkspaceOptions = {
    project,
    setProject,
    nodes,
    edges,
    setNodes,
    setEdges,
    setSelectedId,
    setProjectsOpen,
    setStatus,
    invalidateRun: vi.fn(),
  };
  return { ...useProjectWorkspace(options), nodes };
}

function testProject(): Project {
  return {
    id: PROJECT_ID,
    name: "Alpha",
    revision: "1",
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "1",
  };
}

function workspaceSummary(project: Project, workflowId: string) {
  return {
    project,
    current_workflow_summary: {
      workflow_id: workflowId,
      workflow_revision: "1",
      readiness: "ready" as const,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
