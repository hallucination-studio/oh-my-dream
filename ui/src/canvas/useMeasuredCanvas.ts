import { useLayoutEffect, useRef, useState } from "react";

/** Delays React Flow until its parent has a real non-zero layout box. */
export function useMeasuredCanvas() {
  const canvasRef = useRef<HTMLDivElement>(null);
  const [canvasReady, setCanvasReady] = useState(false);

  useLayoutEffect(() => {
    const element = canvasRef.current;
    if (!element) return;
    const update = () => {
      const bounds = element.getBoundingClientRect();
      setCanvasReady(bounds.width > 0 && bounds.height > 0);
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return { canvasRef, canvasReady };
}
