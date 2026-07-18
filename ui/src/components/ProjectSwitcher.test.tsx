import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type Project } from "../api/index.ts";
import { ProjectSwitcher } from "./ProjectSwitcher.tsx";

afterEach(() => vi.restoreAllMocks());

describe("ProjectSwitcher", () => {
  it("deduplicates by Project ID and refreshes after rename", async () => {
    const alpha = project("alpha", "Alpha", "1");
    const stale = project("alpha", "Stale Alpha", "1");
    const renamed = project("alpha", "Renamed Alpha", "2");
    vi.spyOn(api, "listProjects")
      .mockResolvedValueOnce([stale, alpha])
      .mockResolvedValueOnce([renamed]);
    vi.spyOn(api, "renameProject").mockResolvedValue(renamed);
    const onProjectRenamed = vi.fn();

    render(
      <ProjectSwitcher
        current={alpha}
        open
        onClose={vi.fn()}
        onOpenProject={vi.fn()}
        onProjectRenamed={onProjectRenamed}
      />,
    );

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /Alpha/ })).toHaveLength(1);
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Rename current project" }), {
      target: { value: "Renamed Alpha" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() => expect(onProjectRenamed).toHaveBeenCalledWith(renamed));
    expect(api.renameProject).toHaveBeenCalledWith(alpha, "Renamed Alpha");
    expect(await screen.findByRole("button", { name: /Renamed Alpha/ })).toBeTruthy();
    expect(api.listProjects).toHaveBeenCalledTimes(2);
  });

  it("guides an empty list and surfaces create failures", async () => {
    vi.spyOn(api, "listProjects").mockResolvedValue([]);
    vi.spyOn(api, "createProject").mockRejectedValue(new Error("disk full"));

    render(
      <ProjectSwitcher
        current={null}
        open
        onClose={vi.fn()}
        onOpenProject={vi.fn()}
        onProjectRenamed={vi.fn()}
      />,
    );

    expect(await screen.findByText("No projects yet — create your first one below.")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "New project name" }), {
      target: { value: "First" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Create project failed · disk full");
  });
});

function project(id: string, name: string, revision: string): Project {
  return {
    id,
    name,
    revision,
    created_at_epoch_ms: "0",
    updated_at_epoch_ms: "0",
  };
}
