import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
import type { ProjectWorkspace } from "./api/types.ts";
import { App } from "./App.tsx";
import { deferred, selectProject, workspace } from "./test/appFixtures.ts";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("App", () => {
  it("adds a node from the palette and runs it through the mock API", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: /Default/ })).toBeTruthy());
    vi.useFakeTimers();

    // The palette lists node types; add one, then run the workflow.
    fireEvent.click(screen.getByRole("button", { name: /Text Prompt/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    expect(runState()).toContain("Running");
    act(() => {
      vi.runAllTimers();
    });
    expect(runState()).toContain("Done");
  });

  it("hydrates the initial project and replaces the graph when switching projects", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation(async (id) =>
      id === beta.project.id ? beta : alpha,
    );

    render(<App />);

    await waitFor(() => expect(screen.getByDisplayValue("alpha prompt")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /Alpha/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Beta" }));

    await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());
    expect(screen.queryByDisplayValue("alpha prompt")).toBeNull();
  });

  it("saves the active project snapshot after a normal parameter edit", async () => {
    const alpha = workspace("alpha", "Alpha", "before edit");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const saveWorkflow = vi.spyOn(api, "saveWorkflow").mockResolvedValue();

    render(<App />);

    const prompt = await screen.findByDisplayValue("before edit");
    fireEvent.change(prompt, { target: { value: "after edit" } });

    await waitFor(() =>
      expect(saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          project_id: "alpha",
          nodes: [expect.objectContaining({ params: { text: "after edit" } })],
        }),
      ),
    );
  });

  it("does not surface a failed save after a newer snapshot succeeds", async () => {
    const alpha = workspace("alpha", "Alpha", "initial prompt");
    const firstSave = deferred<void>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const saveWorkflow = vi
      .spyOn(api, "saveWorkflow")
      .mockImplementationOnce(() => firstSave.promise)
      .mockResolvedValueOnce();

    render(<App />);
    const prompt = await screen.findByDisplayValue("initial prompt");
    fireEvent.change(prompt, { target: { value: "first edit" } });
    await waitFor(() => expect(saveWorkflow).toHaveBeenCalledTimes(1));
    fireEvent.change(prompt, { target: { value: "newer edit" } });

    await act(async () => {
      firstSave.reject(new Error("stale save failure"));
      await Promise.resolve();
    });
    await waitFor(() => expect(saveWorkflow).toHaveBeenCalledTimes(2));

    expect(runState()).not.toContain("stale save failure");
  });

  it("does not reopen or rehydrate the current project", async () => {
    const alpha = workspace("alpha", "Alpha", "stored prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    const openProject = vi.spyOn(api, "openProject").mockResolvedValue(alpha);

    render(<App />);
    const prompt = await screen.findByDisplayValue("stored prompt");
    openProject.mockClear();

    fireEvent.click(screen.getByRole("button", { name: /Alpha/ }));
    const alphaButtons = await screen.findAllByRole("button", { name: "Alpha" });
    fireEvent.click(alphaButtons[alphaButtons.length - 1]);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await new Promise((resolve) => window.setTimeout(resolve, 20));
    });
    fireEvent.change(prompt, { target: { value: "kept local edit" } });

    expect(openProject).not.toHaveBeenCalled();
    expect(screen.getByDisplayValue("kept local edit")).toBeTruthy();
  });

  it("saves the current project before hydrating a different project", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    const events: string[] = [];
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation(async (id) => {
      events.push(`open:${id}`);
      return id === "beta" ? beta : alpha;
    });
    vi.spyOn(api, "saveWorkflow").mockImplementation(async (workflow) => {
      const text = String(workflow.nodes[0]?.params.text ?? "");
      events.push(`save:${workflow.project_id}:${text}`);
    });

    render(<App />);
    const prompt = await screen.findByDisplayValue("alpha prompt");
    events.length = 0;
    fireEvent.change(prompt, { target: { value: "edited alpha" } });
    fireEvent.click(screen.getByRole("button", { name: /Alpha/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Beta" }));

    await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());
    expect(events.slice(0, 2)).toEqual(["save:alpha:edited alpha", "open:beta"]);
  });

  it("flushes an edit made while the next project is loading", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    const betaWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    const openProject = vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === "beta" ? betaWorkspace.promise : Promise.resolve(alpha),
    );
    const saveWorkflow = vi.spyOn(api, "saveWorkflow").mockResolvedValue();

    render(<App />);
    const prompt = await screen.findByDisplayValue("alpha prompt");
    openProject.mockClear();

    await selectProject("Alpha", "Beta");
    await waitFor(() => expect(openProject).toHaveBeenCalledWith("beta"));
    fireEvent.change(prompt, { target: { value: "late alpha edit" } });
    betaWorkspace.resolve(beta);

    await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());
    expect(saveWorkflow).toHaveBeenCalledWith(
      expect.objectContaining({
        project_id: "alpha",
        nodes: [expect.objectContaining({ params: { text: "late alpha edit" } })],
      }),
    );
  });

  it("ignores a superseded project-switch failure", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    const gamma = workspace("gamma", "Gamma", "gamma prompt");
    const betaWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([
      alpha.project,
      beta.project,
      gamma.project,
    ]);
    const openProject = vi.spyOn(api, "openProject").mockImplementation((id) => {
      if (id === "beta") return betaWorkspace.promise;
      return Promise.resolve(id === "gamma" ? gamma : alpha);
    });

    render(<App />);
    await screen.findByDisplayValue("alpha prompt");
    openProject.mockClear();

    await selectProject("Alpha", "Beta");
    await waitFor(() => expect(openProject).toHaveBeenCalledWith("beta"));
    await selectProject("Alpha", "Gamma");
    await waitFor(() => expect(screen.getByDisplayValue("gamma prompt")).toBeTruthy());

    await act(async () => {
      betaWorkspace.reject(new Error("stale beta failure"));
      await Promise.resolve();
    });

    expect(runState()).not.toContain("stale beta failure");
  });

  it("ignores a superseded initial-load failure", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    const initialWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === "alpha" ? initialWorkspace.promise : Promise.resolve(beta),
    );

    render(<App />);
    await selectProject("No project", "Beta");
    await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());

    await act(async () => {
      initialWorkspace.reject(new Error("stale initial failure"));
      await Promise.resolve();
    });

    expect(runState()).not.toContain("stale initial failure");
  });

  it("flushes the latest edit when the app unmounts during the debounce window", async () => {
    const alpha = workspace("alpha", "Alpha", "before close");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const saveWorkflow = vi.spyOn(api, "saveWorkflow").mockResolvedValue();
    const view = render(<App />);
    const prompt = await screen.findByDisplayValue("before close");

    fireEvent.change(prompt, { target: { value: "saved on close" } });
    view.unmount();

    await waitFor(() =>
      expect(saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          project_id: "alpha",
          nodes: [expect.objectContaining({ params: { text: "saved on close" } })],
        }),
      ),
    );
  });

  it("does not overwrite a draft created before initial hydration completes", async () => {
    const alpha = workspace("alpha", "Alpha", "stored prompt");
    const initialWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockReturnValue(initialWorkspace.promise);

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /Text Prompt/ }));
    fireEvent.change(screen.getByLabelText("Prompt"), {
      target: { value: "draft before hydration" },
    });
    initialWorkspace.resolve(alpha);

    await waitFor(() => expect(screen.getByRole("button", { name: /Alpha/ })).toBeTruthy());
    expect(screen.getByDisplayValue("draft before hydration")).toBeTruthy();
    expect(screen.queryByDisplayValue("stored prompt")).toBeNull();
  });

  it("adopts an unassigned draft when a project is selected manually", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "stored beta prompt");
    const initialWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation((id) =>
      id === "alpha" ? initialWorkspace.promise : Promise.resolve(beta),
    );

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /Text Prompt/ }));
    fireEvent.change(screen.getByLabelText("Prompt"), {
      target: { value: "draft for selected project" },
    });
    await selectProject("No project", "Beta");

    await waitFor(() => expect(screen.getByRole("button", { name: /Beta/ })).toBeTruthy());
    expect(screen.getByDisplayValue("draft for selected project")).toBeTruthy();
    expect(screen.queryByDisplayValue("stored beta prompt")).toBeNull();
  });

  it("preserves an intentionally emptied draft during initial hydration", async () => {
    const alpha = workspace("alpha", "Alpha", "stored prompt");
    const initialWorkspace = deferred<ProjectWorkspace>();
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockReturnValue(initialWorkspace.promise);

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /Text Prompt/ }));
    const flowNode = document.querySelector<HTMLElement>(".react-flow__node");
    expect(flowNode).not.toBeNull();
    fireEvent.click(flowNode as HTMLElement);
    fireEvent.keyDown(document, { key: "Backspace", code: "Backspace" });
    fireEvent.keyUp(document, { key: "Backspace", code: "Backspace" });
    await waitFor(() => expect(document.querySelector(".react-flow__node")).toBeNull());
    initialWorkspace.resolve(alpha);

    await waitFor(() => expect(screen.getByRole("button", { name: /Alpha/ })).toBeTruthy());
    expect(screen.queryByDisplayValue("stored prompt")).toBeNull();
  });

  it("opens the assistant dock from the rail when enabled", async () => {
    vi.spyOn(api, "getAssistantConfig").mockResolvedValue({
      enabled: true,
      base_url: "https://api.openai.com/v1",
      model: "gpt-5.4",
      has_key: false,
      temperature: 0.3,
      max_tool_iters: 20,
      developer_mode: false,
      system_prompt_extra: null,
      skills: { installed: [], enabled: [] },
    });

    render(<App />);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Assistant" })).not.toBeNull());

    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));

    expect(screen.getByRole("complementary", { name: "Assistant" })).not.toBeNull();
    expect(screen.getByPlaceholderText("Message the assistant")).not.toBeNull();
  });
});

function runState(): string {
  const bar = document.querySelector(".topbar__state");
  return bar?.textContent ?? "";
}
