import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import fixture from "../__fixtures__/generation_provider_settings.json";
import type {
  GenerationProviderSettingsActionDto,
  GenerationProviderSettingsDto,
} from "../api/types.ts";
import { SettingsDialog } from "./SettingsDialog.tsx";

describe("Generation Provider Settings", () => {
  it("renders only Mock bindings and applies remove then set", async () => {
    let settings = structuredClone(fixture) as GenerationProviderSettingsDto;
    const apply = vi.fn(async (
      _revision: string,
      action: GenerationProviderSettingsActionDto,
    ) => {
      const profile = settings.profiles.find((item) => item.profile_ref === action.profile_ref)!;
      profile.selected_binding = action.kind === "remove_binding"
        ? null
        : { provider_id: action.provider_id, route_id: action.route_id };
      settings.settings_revision = String(Number(settings.settings_revision) + 1);
      return structuredClone(settings);
    });
    render(
      <SettingsDialog
        open
        onClose={vi.fn()}
        settingsApi={{
          generationProviderSettingsGet: async () => structuredClone(settings),
          generationProviderSettingsApply: apply,
        }}
      />,
    );

    expect(await screen.findByRole("heading", { name: "Generation models" })).toBeTruthy();
    expect(screen.getAllByRole("checkbox")).toHaveLength(3);
    expect(screen.getAllByRole("combobox")).toHaveLength(3);
    expect(screen.getAllByRole("option", { name: /^Mock ·/ })).toHaveLength(3);
    const text = screen.getByRole("dialog").textContent ?? "";
    expect(text).not.toMatch(/credential|account|endpoint|native model/i);

    fireEvent.click(screen.getByRole("checkbox", { name: "Enable High Quality General Image" }));
    await waitFor(() => expect(apply).toHaveBeenLastCalledWith("1", {
      kind: "remove_binding",
      profile_ref: "image.high_quality_general@1",
      generation_kind: "image",
    }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Enable High Quality General Image" }));
    await waitFor(() => expect(apply).toHaveBeenLastCalledWith("2", {
      kind: "set_binding",
      profile_ref: "image.high_quality_general@1",
      generation_kind: "image",
      provider_id: "mock",
      route_id: "mock.image.high-quality-general.v1",
    }));
  });

  it("reloads on revision conflict and closes with Escape", async () => {
    const latest = structuredClone(fixture) as GenerationProviderSettingsDto;
    latest.settings_revision = "2";
    latest.profiles[0]!.selected_binding = null;
    const get = vi.fn()
      .mockResolvedValueOnce(structuredClone(fixture))
      .mockResolvedValueOnce(latest);
    const close = vi.fn();
    render(
      <SettingsDialog
        open
        onClose={close}
        settingsApi={{
          generationProviderSettingsGet: get,
          generationProviderSettingsApply: vi.fn().mockRejectedValue(
            new Error("generation_provider_settings.revision_conflict"),
          ),
        }}
      />,
    );

    const checkbox = await screen.findByRole("checkbox", {
      name: "Enable High Quality General Image",
    });
    fireEvent.click(checkbox);
    expect(await screen.findByText("Settings changed. Latest values loaded.")).toBeTruthy();
    expect(get).toHaveBeenCalledTimes(2);
    expect((checkbox as HTMLInputElement).checked).toBe(false);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });
});
