import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
import type { RunObserver } from "./api/types.ts";
import { App } from "./App.tsx";
import { selectProject, workspace } from "./test/appFixtures.ts";

afterEach(() => {
  vi.restoreAllMocks();
});

it("keeps a new project run active when the previous run reports a late terminal state", async () => {
  const alpha = workspace("alpha", "Alpha", "alpha prompt");
  const beta = workspace("beta", "Beta", "beta prompt");
  vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
  vi.spyOn(api, "openProject").mockImplementation(async (id) => (id === "beta" ? beta : alpha));
  const alphaCancel = vi.fn();
  const betaCancel = vi.fn();
  const observers: RunObserver[] = [];
  vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
    observers.push(next);
    const alphaRun = observers.length === 1;
    return {
      runId: alphaRun ? "alpha-run" : "beta-run",
      cancel: alphaRun ? alphaCancel : betaCancel,
    };
  });

  render(<App />);
  await selectProject("No project", "Alpha");
  await screen.findByDisplayValue("alpha prompt");
  await startRun();
  await selectProject("Alpha", "Beta");
  await waitFor(() => expect(screen.getByDisplayValue("beta prompt")).toBeTruthy());

  expect(alphaCancel).toHaveBeenCalledTimes(1);
  await startRun();
  act(() => observers[0]?.onStatus({ state: "cancelled" }));

  expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
  expect(runState()).toContain("Running");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(betaCancel).toHaveBeenCalledTimes(1);
});

it("cancels the active run and clears its projection when the workflow is edited", async () => {
  const alpha = workspace("alpha", "Alpha", "alpha prompt");
  vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
  vi.spyOn(api, "openProject").mockResolvedValue(alpha);
  const cancel = vi.fn();
  let observe: RunObserver | null = null;
  vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
    observe = next;
    return { runId: "alpha-run", cancel };
  });

  render(<App />);
  await selectProject("No project", "Alpha");
  const prompt = await screen.findByDisplayValue("alpha prompt");
  await startRun();
  act(() => {
    const runObserver = observe as RunObserver | null;
    runObserver?.onProgress({
      nodeId: "alpha-prompt",
      progress: 0.5,
      nodeState: "running",
    });
  });
  expect(document.querySelector(".wf-node.is-running")).not.toBeNull();

  fireEvent.change(prompt, { target: { value: "edited during run" } });

  expect(cancel).toHaveBeenCalledTimes(1);
  expect(document.querySelector(".wf-node.is-idle")).not.toBeNull();
  expect(runState()).not.toContain("Running");

  act(() => observe?.onStatus({
    state: "succeeded",
    outputs: { "alpha-prompt": { text: { kind: "string", value: "stale" } } },
  }));
  expect(document.querySelector(".wf-node.is-done")).toBeNull();
  expect(runState()).not.toContain("Done");
});

it("clears a completed run projection when the workflow is edited", async () => {
  const alpha = workspace("alpha", "Alpha", "alpha prompt");
  vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
  vi.spyOn(api, "openProject").mockResolvedValue(alpha);
  const cancel = vi.fn();
  let observe: RunObserver | null = null;
  vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
    observe = next;
    return { runId: "alpha-run", cancel };
  });

  render(<App />);
  await selectProject("No project", "Alpha");
  const prompt = await screen.findByDisplayValue("alpha prompt");
  await startRun();
  act(() => {
    const runObserver = observe as RunObserver | null;
    runObserver?.onStatus({
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
  vi.spyOn(api, "runWorkflow").mockReturnValue({ runId: "alpha-run", cancel });
  const view = render(<App />);
  await selectProject("No project", "Alpha");
  await screen.findByDisplayValue("alpha prompt");
  await startRun();

  view.unmount();

  expect(cancel).toHaveBeenCalledTimes(1);
});

it("shows cancelling until the run reports an authoritative terminal state", async () => {
  const alpha = workspace("alpha", "Alpha", "alpha prompt");
  vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
  vi.spyOn(api, "openProject").mockResolvedValue(alpha);
  let observe: RunObserver | null = null;
  const cancel = vi.fn(() => observe?.onStatus({ state: "cancelling" }));
  vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
    observe = next;
    return { runId: "alpha-run", cancel };
  });

  render(<App />);
  await selectProject("No project", "Alpha");
  await screen.findByDisplayValue("alpha prompt");
  await startRun();
  act(() => observe?.onProgress({
    nodeId: "alpha-prompt",
    progress: 0.5,
    nodeState: "running",
  }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

  expect(cancel).toHaveBeenCalledTimes(1);
  expect(document.querySelector(".wf-node.is-running")).not.toBeNull();
  expect((screen.getByRole("button", { name: "Cancelling…" }) as HTMLButtonElement).disabled)
    .toBe(true);
  expect(runState()).toContain("Cancelling");

  act(() => observe?.onProgress({
    nodeId: "alpha-prompt",
    progress: 1,
    nodeState: "done",
    cost: 7,
  }));
  expect(document.querySelector(".wf-node.is-done")).not.toBeNull();
  expect(runState()).toContain("Cancelling");

  act(() => observe?.onStatus({ state: "cancelled" }));

  expect(screen.getByRole("button", { name: "Run" })).toBeTruthy();
  expect(document.querySelector(".wf-node.is-done")).not.toBeNull();
  expect(screen.getByRole("status").textContent).toContain("Cancelled");
});

it("keeps the active run cancellable after a cancellation request fails", async () => {
  const alpha = workspace("alpha", "Alpha", "alpha prompt");
  vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project]);
  vi.spyOn(api, "openProject").mockResolvedValue(alpha);
  let observe: RunObserver | null = null;
  const cancel = vi.fn(() => {
    observe?.onStatus({ state: "cancel_failed", reason: "cancel transport failed" });
  });
  vi.spyOn(api, "runWorkflow").mockImplementation((_workflow, next) => {
    observe = next;
    return { runId: "alpha-run", cancel };
  });

  render(<App />);
  await selectProject("No project", "Alpha");
  await screen.findByDisplayValue("alpha prompt");
  await startRun();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

  expect(screen.getByRole("status").textContent).toContain("Cancel request failed");
  fireEvent.click(screen.getByRole("button", { name: "Retry Cancel" }));
  expect(cancel).toHaveBeenCalledTimes(2);
});

function runState(): string {
  return document.querySelector(".topbar__state")?.textContent ?? "";
}

async function startRun(): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: "Run" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy());
}
