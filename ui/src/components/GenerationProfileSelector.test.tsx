import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import { GenerationProfileSelector } from "./GenerationProfileSelector.tsx";

afterEach(() => vi.restoreAllMocks());

it("preserves backend order and disables unavailable profiles", async () => {
  vi.spyOn(api, "generationProfileListForCapability").mockResolvedValue([
    {
      profile_ref: "image.high_quality_general@1",
      display_name: "High Quality Image",
      availability: {
        state: "unavailable",
        reason: "authentication_required",
        retry_after_epoch_ms: null,
        observed_at_epoch_ms: "1",
        expires_at_epoch_ms: "2",
      },
    },
  ]);
  const onChange = vi.fn();

  render(
    <GenerationProfileSelector
      capability={{ id: "image.generate_from_text", version: "1.0" }}
      value=""
      onChange={onChange}
    />,
  );

  const option = await screen.findByRole("option", { name: /High Quality Image/ });
  expect((option as HTMLOptionElement).disabled).toBe(true);
  fireEvent.change(screen.getByRole("combobox", { name: "Generation profile" }), {
    target: { value: "" },
  });
  expect(api.generationProfileListForCapability).toHaveBeenCalledWith({
    id: "image.generate_from_text",
    version: "1.0",
  });
});
