import { useLayoutEffect, useRef, useState } from "react";

/** Delays React Flow until its parent has a real non-zero layout box. */
export function useMeasuredCanvas() {
  const canvasRef = useRef<HTMLDivElement>(null);
  const [canvasReady, setCanvasReady] = useState(false);
  const readyRef = useRef(false);

  useLayoutEffect(() => {
    const element = canvasRef.current;
    if (!element) return;
    const update = () => {
      const bounds = element.getBoundingClientRect();
      const ready = bounds.width > 0 && bounds.height > 0;
      if (ready !== readyRef.current) {
        readyRef.current = ready;
        setCanvasReady(ready);
      }
    };
    update();
    let frame: number | undefined;
    const observer = new ResizeObserver(() => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    });
    observer.observe(element);
    return () => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, []);

  return { canvasRef, canvasReady };
}
