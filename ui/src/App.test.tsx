import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App.tsx";
import { api } from "./api/index.ts";

afterEach(() => vi.restoreAllMocks());

describe("App canonical workspace shell", () => {
  it("keeps the frozen shell available without an active Project", async () => {
    vi.spyOn(api, "listProjects").mockResolvedValue([]);
    render(<App />);

    await waitFor(() => expect(screen.getByText("No project")).toBeTruthy());
    expect(screen.getByRole("button", { name: "Run all" })).toBeTruthy();
    expect(screen.getByText("Nodes")).toBeTruthy();
  });

  it("guides the empty canvas and blocks an empty run", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByText("No project")).toBeTruthy());
    expect(
      screen.getByText("Create or open a Project from the top bar to start building."),
    ).toBeTruthy();

    const startRun = vi.spyOn(api, "workflowStartRun");
    fireEvent.click(screen.getByRole("button", { name: "No project" }));
    fireEvent.change(screen.getByRole("textbox", { name: "New project name" }), {
      target: { value: "First" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    await waitFor(() =>
      expect(screen.getByText("Add a Text node from the library to begin.")).toBeTruthy(),
    );

    const runAll = screen.getByRole("button", { name: "Run all" });
    await waitFor(() => expect((runAll as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(runAll);
    expect(
      await screen.findByText("Add a node from the library before running"),
    ).toBeTruthy();
    expect(startRun).not.toHaveBeenCalled();
  });

  it("keeps the assistant dock alive across close and reopen", async () => {
    vi.spyOn(api, "listProjects").mockResolvedValue([]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("No project")).toBeTruthy());

    const rail = screen.getByRole("button", { name: "Assistant" });
    fireEvent.click(rail);
    await waitFor(() => expect(document.querySelector(".adock-host")?.hasAttribute("hidden")).toBe(false));
    expect(screen.getByPlaceholderText("Message the assistant")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Close assistant" }));
    const host = document.querySelector(".adock-host");
    expect(host?.hasAttribute("hidden")).toBe(true);
    // The dock is still mounted — its composer never left the DOM.
    expect(document.querySelector("[name='assistant-message']")).not.toBeNull();

    fireEvent.click(rail);
    expect(host?.hasAttribute("hidden")).toBe(false);
  });
});
