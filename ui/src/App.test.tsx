import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
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
