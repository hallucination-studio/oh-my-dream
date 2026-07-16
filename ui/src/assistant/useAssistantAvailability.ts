import { useCallback, useEffect, useState } from "react";
import { api } from "../api/index.ts";

export function useAssistantAvailability() {
  const [assistantEnabled, setAssistantEnabled] = useState(false);
  const [assistantOpen, setAssistantOpen] = useState(false);
  const refreshAssistantEnabled = useCallback(() => {
    void api
      .getAssistantConfig()
      .then((config) => setAssistantEnabled(config.enabled))
      .catch(() => setAssistantEnabled(false));
  }, []);
  useEffect(refreshAssistantEnabled, [refreshAssistantEnabled]);
  useEffect(() => {
    if (!assistantEnabled) setAssistantOpen(false);
  }, [assistantEnabled]);
  return {
    assistantEnabled,
    assistantOpen,
    setAssistantOpen,
    refreshAssistantEnabled,
  };
}
