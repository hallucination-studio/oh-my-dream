import { useCallback, useEffect, useState } from "react";
import { api, type WorkflowApi } from "../api/index.ts";

export type AssistantAvailability = "loading" | "enabled" | "disabled" | "error";

/// Assistant availability is enforced by the canonical command boundary.
export function useAssistantAvailability(
  apiClient: Pick<WorkflowApi, "assistantProviderSettingsGet"> = api,
) {
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [assistantAvailability, setAssistantAvailability] =
    useState<AssistantAvailability>("loading");
  const getSettings = apiClient.assistantProviderSettingsGet;
  const refreshAssistantEnabled = useCallback(async () => {
    setAssistantAvailability("loading");
    try {
      const settings = await getSettings();
      setAssistantAvailability(settings.enabled ? "enabled" : "disabled");
      if (!settings.enabled) setAssistantOpen(false);
    } catch {
      setAssistantAvailability("error");
      setAssistantOpen(false);
    }
  }, [getSettings]);

  useEffect(() => {
    void refreshAssistantEnabled();
  }, [refreshAssistantEnabled]);

  return {
    assistantEnabled: assistantAvailability === "enabled",
    assistantAvailability,
    assistantOpen,
    setAssistantOpen,
    refreshAssistantEnabled,
  };
}
