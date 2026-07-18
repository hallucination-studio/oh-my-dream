import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { TopBar } from "./TopBar.tsx";

it("renders the Run all action and an honest succeeded summary", () => {
  const { unmount } = render(
    <TopBar
      project={null}
      status={{
        state: "succeeded",
        steps: 3,
        outputs: {
          "node-a": { image: { kind: "image", value: "desktop-asset://v1/a" } },
          "node-b": { video: { kind: "video", value: "desktop-asset://v1/b" } },
        },
      }}
      workspaceState={{ state: "no_project" }}
      onOpenProjects={vi.fn()}
      onOpenSettings={vi.fn()}
      onRun={vi.fn()}
      onCancel={vi.fn()}
      onOpenRunDetails={vi.fn()}
      hasRunDetails={false}
    />,
  );
  expect(screen.getByRole("button", { name: "Run all" })).toBeTruthy();
  expect(screen.getByText("3 steps complete · 2 assets created")).toBeTruthy();
  unmount();
});

it("uses singular forms and omits the assets clause at zero", () => {
  render(
    <TopBar
      project={null}
      status={{
        state: "succeeded",
        steps: 1,
        outputs: { "node-a": { text: { kind: "string", value: "hi" } } },
      }}
      workspaceState={{ state: "no_project" }}
      onOpenProjects={vi.fn()}
      onOpenSettings={vi.fn()}
      onRun={vi.fn()}
      onCancel={vi.fn()}
      onOpenRunDetails={vi.fn()}
      hasRunDetails={false}
    />,
  );
  expect(screen.getByText("1 step complete")).toBeTruthy();
  expect(screen.queryByText(/outputs/)).toBeNull();
});
