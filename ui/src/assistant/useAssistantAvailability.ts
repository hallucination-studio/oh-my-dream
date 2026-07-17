import { useState } from "react";

/// Assistant availability is enforced by the canonical command boundary.
export function useAssistantAvailability() {
  const [assistantOpen, setAssistantOpen] = useState(false);
  return {
    assistantEnabled: true,
    assistantOpen,
    setAssistantOpen,
    refreshAssistantEnabled: () => {},
  };
}
