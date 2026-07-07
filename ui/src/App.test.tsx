import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App.tsx";

describe("App", () => {
  it("adds a node and runs it through the mock API", () => {
    vi.useFakeTimers();
    const { container } = render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "+ Text Prompt" }));
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    expect(statusText(container)).toContain("Running n1");
    act(() => {
      vi.runAllTimers();
    });
    expect(statusText(container)).toContain("Done");
    vi.useRealTimers();
  });
});

function statusText(container: HTMLElement): string {
  return container.querySelector(".status")?.textContent ?? "";
}
