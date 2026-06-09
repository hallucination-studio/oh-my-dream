import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nowIso, uid } from "../fixtures";
import type { GenerationHistory } from "../types";

export function useCanvasHistoryActions({
  setHistory
}: {
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
}) {
  const addHistory = useCallback(
    (item: Omit<GenerationHistory, "id" | "createdAt">) => {
      const record: GenerationHistory = { id: uid("history"), createdAt: nowIso(), ...item };
      setHistory((items) => [record, ...items]);
      return record;
    },
    [setHistory]
  );

  return { addHistory };
}
