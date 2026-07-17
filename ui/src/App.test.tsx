import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App.tsx";
import { api } from "./api/index.ts";

afterEach(() => vi.restoreAllMocks());

describe("App canonical workspace shell", () => {
  it("keeps the frozen shell available without an active Project", async () => {
    vi.spyOn(api, "listProjects").mockResolvedValue([]);
    render(<App />);

    await waitFor(() => expect(screen.getByText("No project")).toBeTruthy());
    expect(screen.getByRole("button", { name: "Run" })).toBeTruthy();
    expect(screen.getByText("Nodes")).toBeTruthy();
  });
});
