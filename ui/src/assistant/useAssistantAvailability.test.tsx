import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useAssistantAvailability } from "./useAssistantAvailability.ts";

describe("useAssistantAvailability", () => {
  it("reports configured, disabled, and failed-load states truthfully", async () => {
    const get = vi.fn()
      .mockResolvedValueOnce({ enabled: true })
      .mockResolvedValueOnce({ enabled: false })
      .mockRejectedValueOnce(new Error("storage.unavailable"));
    const { result } = renderHook(() => useAssistantAvailability({
      assistantProviderSettingsGet: get,
    }));

    await waitFor(() => expect(result.current.assistantAvailability).toBe("enabled"));
    await act(() => result.current.refreshAssistantEnabled());
    expect(result.current.assistantAvailability).toBe("disabled");
    await act(() => result.current.refreshAssistantEnabled());
    expect(result.current.assistantAvailability).toBe("error");
  });
});
