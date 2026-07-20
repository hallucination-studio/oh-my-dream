import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import fixture from "../__fixtures__/generation_provider_settings.json";
import type {
  AssistantProviderSettingsDto,
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
          ...assistantApi(),
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
          ...assistantApi(),
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

describe("Assistant Provider Settings", () => {
  it("discovers a text model and saves only through the compatibility test", async () => {
    const get = vi.fn().mockResolvedValue(disabledAssistantSettings());
    const list = vi.fn().mockResolvedValue({ models: ["model-a", "model-b"] });
    const apply = vi.fn().mockResolvedValue({
      ...disabledAssistantSettings(),
      settings_revision: "2",
      enabled: true,
      model_id: "model-a",
      has_api_key: true,
    });
    render(
      <SettingsDialog
        open
        onClose={vi.fn()}
        settingsApi={{
          generationProviderSettingsGet: async () =>
            structuredClone(fixture) as GenerationProviderSettingsDto,
          generationProviderSettingsApply: vi.fn(),
          assistantProviderSettingsGet: get,
          assistantProviderModelsList: list,
          assistantProviderSettingsTestAndApply: apply,
          assistantProviderSettingsDisable: vi.fn(),
        }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));
    const baseUrl = await screen.findByRole("textbox", { name: "Base URL" });
    const apiKey = screen.getByLabelText("API key") as HTMLInputElement;
    fireEvent.change(baseUrl, { target: { value: "http://localhost:11434/v1" } });
    fireEvent.change(apiKey, { target: { value: "secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Load models" }));

    await waitFor(() => expect(list).toHaveBeenCalledWith(
      "http://localhost:11434/v1",
      "secret",
    ));
    const model = await screen.findByRole("combobox", { name: "Model" });
    fireEvent.change(model, { target: { value: "model-a" } });
    fireEvent.click(screen.getByRole("button", { name: "Test and save" }));

    await waitFor(() => expect(apply).toHaveBeenCalledWith(
      "1",
      "http://localhost:11434/v1",
      "secret",
      "model-a",
    ));
    expect(await screen.findByText("Assistant connected.")).toBeTruthy();
    expect(apiKey.value).toBe("");
    expect(screen.getByRole("switch", { name: "Enable Assistant" }).getAttribute("aria-checked"))
      .toBe("true");
  });

  it("invalidates discovered models on connection edits and clears the key when closed", async () => {
    const close = vi.fn();
    const view = render(
      <SettingsDialog open onClose={close} settingsApi={assistantApi()} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));
    const key = await screen.findByLabelText("API key") as HTMLInputElement;
    fireEvent.change(key, { target: { value: "draft-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Load models" }));
    expect(await screen.findByRole("combobox", { name: "Model" })).toBeTruthy();

    fireEvent.change(screen.getByRole("textbox", { name: "Base URL" }), {
      target: { value: "http://localhost:11435/v1" },
    });
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();

    view.rerender(<SettingsDialog open={false} onClose={close} settingsApi={assistantApi()} />);
    view.rerender(<SettingsDialog open onClose={close} settingsApi={assistantApi()} />);
    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));
    expect((await screen.findByLabelText("API key") as HTMLInputElement).value).toBe("");
  });

  it("disables a configured Assistant through the dedicated command", async () => {
    const configured = { ...disabledAssistantSettings(), enabled: true, has_api_key: true };
    const disable = vi.fn().mockResolvedValue({
      ...configured,
      settings_revision: "2",
      enabled: false,
    });
    render(
      <SettingsDialog
        open
        onClose={vi.fn()}
        settingsApi={{ ...assistantApi(configured), assistantProviderSettingsDisable: disable }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));
    const toggle = await screen.findByRole("switch", { name: "Enable Assistant" });
    fireEvent.click(toggle);
    await waitFor(() => expect(disable).toHaveBeenCalledWith("1"));
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });
});

function disabledAssistantSettings(): AssistantProviderSettingsDto {
  return {
    settings_revision: "1",
    enabled: false,
    base_url: "https://api.openai.com/v1",
    model_id: null,
    has_api_key: false,
  };
}

function assistantApi(settings = disabledAssistantSettings()) {
  return {
    generationProviderSettingsGet: vi.fn().mockResolvedValue(structuredClone(fixture)),
    generationProviderSettingsApply: vi.fn(),
    assistantProviderSettingsGet: vi.fn().mockResolvedValue(structuredClone(settings)),
    assistantProviderModelsList: vi.fn().mockResolvedValue({ models: ["model-a"] }),
    assistantProviderSettingsTestAndApply: vi.fn(),
    assistantProviderSettingsDisable: vi.fn(),
  };
}
