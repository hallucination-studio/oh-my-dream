import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
import type { RunObserver } from "./api/types.ts";
import { App } from "./App.tsx";
import { selectProject, workspace } from "./test/appFixtures.ts";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("App run lifecycle", () => {
  it("cancels and ignores the previous project's run observer", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    const beta = workspace("beta", "Beta", "beta prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "openProject").mockImplementation(async (id) => (id === "beta" ? beta : alpha));
    const cancel = vi.fn();
    let observe: RunObserver | null = null;
    vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
      observe = next;
      return { cancel };
    });

    render(<App />);
    await screen.findByDisplayValue("alpha prompt");
    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    await selectProject("Alpha", "Beta");
    await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());

    expect(cancel).toHaveBeenCalledTimes(1);
    act(() => {
      const staleObserver = observe as RunObserver | null;
      staleObserver?.({ state: "running", nodeId: "alpha-prompt", progress: 0.75 });
    });
    expect(runState()).not.toContain("Running");
  });

  it("cancels the active run and clears its projection when the workflow is edited", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const cancel = vi.fn();
    let observe: RunObserver | null = null;
    vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
      observe = next;
      return { cancel };
    });

    render(<App />);
    const prompt = await screen.findByDisplayValue("alpha prompt");
    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    act(() => {
      const runObserver = observe as RunObserver | null;
      runObserver?.({ state: "running", nodeId: "alpha-prompt", progress: 0.5 });
    });
    expect(document.querySelector(".wf-node.is-running")).not.toBeNull();

    fireEvent.change(prompt, { target: { value: "edited during run" } });

    expect(cancel).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".wf-node.is-idle")).not.toBeNull();
    expect(runState()).not.toContain("Running");
  });

  it("clears a completed run projection when the workflow is edited", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const cancel = vi.fn();
    let observe: RunObserver | null = null;
    vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
      observe = next;
      return { cancel };
    });

    render(<App />);
    const prompt = await screen.findByDisplayValue("alpha prompt");
    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    act(() => {
      const runObserver = observe as RunObserver | null;
      runObserver?.({
        state: "succeeded",
        outputs: { "alpha-prompt": { text: { kind: "string", value: "result" } } },
      });
    });
    expect(document.querySelector(".wf-node.is-done")).not.toBeNull();

    fireEvent.change(prompt, { target: { value: "edited after run" } });

    expect(cancel).not.toHaveBeenCalled();
    expect(document.querySelector(".wf-node.is-idle")).not.toBeNull();
    expect(runState()).not.toContain("Done");
  });

  it("cancels the active run before unmounting", async () => {
    const alpha = workspace("alpha", "Alpha", "alpha prompt");
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
    vi.spyOn(api, "openProject").mockResolvedValue(alpha);
    const cancel = vi.fn();
    vi.spyOn(api, "runWorkflow").mockReturnValue({ cancel });
    const view = render(<App />);
    await screen.findByDisplayValue("alpha prompt");
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    view.unmount();

    expect(cancel).toHaveBeenCalledTimes(1);
  });
});

function runState(): string {
  return document.querySelector(".topbar__state")?.textContent ?? "";
}
