// Transient editor notices: short-lived messages for editing actions (a
// rejected connection, an unavailable asset), kept out of Run status.

import { useCallback, useEffect, useRef, useState } from "react";

export function useNotice(timeoutMs = 4200) {
  const [notice, setNotice] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  const notify = useCallback(
    (message: string) => {
      setNotice(message);
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setNotice(null), timeoutMs);
    },
    [timeoutMs],
  );

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );

  return { notice, notify };
}
