import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App.tsx";

describe("App", () => {
  it("adds a node from the palette and runs it through the mock API", () => {
    vi.useFakeTimers();
    render(<App />);

    // The palette lists node types; add one, then run the workflow.
    fireEvent.click(screen.getByRole("button", { name: /Text Prompt/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    expect(runState()).toContain("Running");
    act(() => {
      vi.runAllTimers();
    });
    expect(runState()).toContain("Done");
    vi.useRealTimers();
  });
});

function runState(): string {
  const bar = document.querySelector(".topbar__state");
  return bar?.textContent ?? "";
}
