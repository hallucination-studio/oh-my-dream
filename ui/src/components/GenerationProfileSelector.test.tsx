import { render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import { GenerationProfileSelector } from "./GenerationProfileSelector.tsx";

afterEach(() => vi.restoreAllMocks());

it("shows the structured reason for a single unavailable model", async () => {
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

  expect(await screen.findByText("High Quality Image")).toBeTruthy();
  expect(screen.getByText("authentication_required")).toBeTruthy();
  expect(api.generationProfileListForCapability).toHaveBeenCalledWith({
    id: "image.generate_from_text",
    version: "1.0",
  });
});

it("auto-selects one available model and removes empty compatibility", async () => {
  const onChange = vi.fn();
  vi.spyOn(api, "generationProfileListForCapability").mockResolvedValueOnce([{
    profile_ref: "image.high_quality_general@1",
    display_name: "Fast image model",
    availability: {
      state: "available",
      reason: null,
      retry_after_epoch_ms: null,
      observed_at_epoch_ms: "1",
      expires_at_epoch_ms: "2",
    },
  }]).mockResolvedValueOnce([]);

  const { rerender } = render(
    <GenerationProfileSelector
      capability={{ id: "image.generate_from_text", version: "1.0" }}
      value=""
      onChange={onChange}
    />,
  );
  expect(await screen.findByText("Fast image model")).toBeTruthy();
  expect(onChange).toHaveBeenCalledWith("image.high_quality_general@1");

  rerender(
    <GenerationProfileSelector
      capability={{ id: "video.generate_from_image", version: "1.0" }}
      value=""
      onChange={onChange}
    />,
  );
  expect(await screen.findByText("No generation model supports this node type")).toBeTruthy();
  expect(screen.queryByRole("combobox")).toBeNull();
});

it("keeps unavailable and indeterminate models disabled in a multi-model selector", async () => {
  vi.spyOn(api, "generationProfileListForCapability").mockResolvedValue([
    {
      profile_ref: "image.available@1",
      display_name: "Available image",
      availability: {
        state: "available",
        reason: null,
        retry_after_epoch_ms: null,
        observed_at_epoch_ms: "1",
        expires_at_epoch_ms: "2",
      },
    },
    {
      profile_ref: "image.unavailable@1",
      display_name: "Unavailable image",
      availability: {
        state: "unavailable",
        reason: "authentication_required",
        retry_after_epoch_ms: null,
        observed_at_epoch_ms: "1",
        expires_at_epoch_ms: "2",
      },
    },
    {
      profile_ref: "image.indeterminate@1",
      display_name: "Indeterminate image",
      availability: {
        state: "indeterminate",
        reason: "availability_timeout",
        retry_after_epoch_ms: null,
        observed_at_epoch_ms: "1",
        expires_at_epoch_ms: "2",
      },
    },
  ]);

  render(
    <GenerationProfileSelector
      capability={{ id: "image.generate_from_text", version: "1.0" }}
      value=""
      onChange={vi.fn()}
    />,
  );

  const unavailable = await screen.findByRole("option", { name: /Unavailable image/ });
  const indeterminate = screen.getByRole("option", { name: /Indeterminate image/ });
  expect((unavailable as HTMLOptionElement).disabled).toBe(true);
  expect((indeterminate as HTMLOptionElement).disabled).toBe(true);
  expect(unavailable.textContent).toContain("authentication_required");
  expect(indeterminate.textContent).toContain("availability_timeout");
});
