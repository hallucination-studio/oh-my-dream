import { act, renderHook, waitFor } from "@testing-library/react";
import type { Edge, Node } from "@xyflow/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import type { Project, ProjectWorkspace } from "../api/types.ts";
import { deferred, workspace } from "../test/appFixtures.ts";
import { CapabilityContractCache } from "./contractCache.ts";
import { useProjectWorkspace, type ProjectWorkspaceOptions } from "./useProjectWorkspace.ts";
import type { RunStatus } from "./types.ts";
import { useRef, useState } from "react";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useProjectWorkspace", () => {
  it("enters no_project without opening the first listed project", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const openProject = vi.spyOn(api, "openProject");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);

    const view = renderHook(() => useWorkspaceHarness());

    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));
    expect(openProject).not.toHaveBeenCalled();
  });

  it("hydrates an optional head and keeps an empty project distinct", async () => {
    const alpha = emptyWorkspace("alpha", "Alpha");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);

    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));

    act(() => view.result.current.openProject("alpha"));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    expect(view.result.current.workspaceState).toMatchObject({
      state: "ready",
      project: alpha.project,
      workflowHead: null,
    });
  });

  it("ignores a stale open response after a newer project wins", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    const betaOpen = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === "beta" ? betaOpen.promise : Promise.resolve(alpha),
    );

    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));

    act(() => view.result.current.openProject("beta"));
    act(() => view.result.current.openProject("alpha"));
    await waitFor(() => expect(view.result.current.workspaceState).toMatchObject({
      state: "ready",
      project: alpha.project,
    }));

    await act(async () => {
      betaOpen.reject(new Error("stale beta failure"));
      await Promise.resolve();
    });

    expect(view.result.current.workspaceState).toMatchObject({
      state: "ready",
      project: alpha.project,
    });
  });

  it("blocks a failed switch while preserving the prior project", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === "beta" ? Promise.reject(new Error("beta unavailable")) : Promise.resolve(alpha),
    );

    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));
    act(() => view.result.current.openProject("alpha"));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    act(() => view.result.current.openProject("beta"));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("blocked"));

    expect(view.result.current.workspaceState).toMatchObject({
      state: "blocked",
      project: alpha.project,
      reason: "Error: beta unavailable",
    });
  });

  it("keeps the active project editable after a patch flush failure", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const applyWorkflowPatch = vi
      .spyOn(api, "applyWorkflowPatch")
      .mockRejectedValue(new Error("WORKFLOW_REVISION_CONFLICT"));

    const view = renderHook(() => useWorkspaceHarness());
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("no_project"));
    act(() => view.result.current.openProject("alpha"));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    act(() => view.result.current.setParam("alpha-prompt", "text", "edited"));
    await waitFor(() => expect(applyWorkflowPatch).toHaveBeenCalledTimes(1));
    act(() => view.result.current.openProject("alpha"));
    await waitFor(() => expect(view.result.current.workspaceState.state).toBe("ready"));

    expect(view.result.current.workspaceState.state).toBe("ready");
    expect(view.result.current.canEdit).toBe(true);
  });
});

function useWorkspaceHarness() {
  const [project, setProject] = useState<Project | null>(null);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [, setSelectedId] = useState<string | null>(null);
  const [, setProjectsOpen] = useState(false);
  const [, setStatus] = useState<RunStatus>({ state: "idle" });
  const cacheRef = useRef<CapabilityContractCache | null>(null);
  if (cacheRef.current === null) cacheRef.current = new CapabilityContractCache(api);
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
    capabilityCache: cacheRef.current,
  };
  return useProjectWorkspace(options);
}

function emptyWorkspace(id: string, name: string): ProjectWorkspace {
  return {
    project: { id, name, created_at: 0 },
    workflow_head: null,
  };
}
