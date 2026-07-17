import { render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api, type NodeCapabilityContractDto } from "../api/index.ts";
import contracts from "../__fixtures__/node_capabilities.json";
import { useNodeAvailability } from "./useNodeAvailability.ts";

afterEach(() => vi.restoreAllMocks());

it("projects authoritative profile results without querying pure capabilities", async () => {
  const exact = contracts as NodeCapabilityContractDto[];
  const profileQuery = vi.spyOn(api, "generationProfileListForCapability").mockImplementation(async (reference) =>
    reference.id === "image.generate_from_text"
      ? [{
          profile_ref: "image.high_quality_general@1",
          display_name: "Image",
          availability: {
            state: "available",
            reason: null,
            retry_after_epoch_ms: null,
            observed_at_epoch_ms: "1",
            expires_at_epoch_ms: "2",
          },
        }]
      : [],
  );

  render(<AvailabilityProbe contracts={exact} />);

  expect((await screen.findByTestId("availability-image.generate_from_text")).textContent)
    .toBe("image.generate_from_text:available:visible:image.high_quality_general@1");
  expect((await screen.findByTestId("availability-video.generate_from_image")).textContent)
    .toBe("video.generate_from_image:unavailable:hidden:");
  expect(profileQuery).toHaveBeenCalledWith({ id: "image.generate_from_text", version: "1.0" });
  expect(profileQuery).toHaveBeenCalledWith({ id: "video.generate_from_image", version: "1.0" });
  expect(profileQuery).toHaveBeenCalledWith({ id: "audio.synthesize_speech_from_text", version: "1.0" });
  expect(profileQuery).not.toHaveBeenCalledWith({ id: "text.provide_literal", version: "1.0" });
});

function AvailabilityProbe({ contracts }: { contracts: NodeCapabilityContractDto[] }) {
  const { specs, hiddenCapabilityKeys } = useNodeAvailability(contracts);
  return (
    <div>
      {specs.map((spec) => (
        <span key={spec.ref.id} data-testid={`availability-${spec.ref.id}`}>
          {availabilityDescription(spec, hiddenCapabilityKeys)}
        </span>
      ))}
    </div>
  );
}

function availabilityDescription(
  spec: ReturnType<typeof useNodeAvailability>["specs"][number],
  hiddenCapabilityKeys: ReadonlySet<string>,
): string {
  const key = `${spec.ref.id}@${spec.ref.version}`;
  const defaultProfile = spec.params.find((param) => param.name === "generation_profile_ref")?.default;
  return `${spec.ref.id}:${spec.status.availability}:${hiddenCapabilityKeys.has(key) ? "hidden" : "visible"}:${String(defaultProfile ?? "")}`;
}
