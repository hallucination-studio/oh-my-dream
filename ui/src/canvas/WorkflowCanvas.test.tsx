import { render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { WorkflowCanvas } from "./WorkflowCanvas.tsx";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("mounts React Flow only after the parent has non-zero dimensions", () => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: 800,
    bottom: 600,
    width: 800,
    height: 600,
    toJSON: () => ({}),
  });
  const warning = vi.spyOn(console, "warn").mockImplementation(() => undefined);
  const error = vi.spyOn(console, "error").mockImplementation(() => undefined);

  const view = render(
    <WorkflowCanvas
      nodes={[]}
      edges={[]}
      onNodesChange={vi.fn()}
      onEdgesChange={vi.fn()}
      onConnect={vi.fn()}
      onConnectStart={vi.fn()}
      onConnectEnd={vi.fn()}
      isValidConnection={() => true}
      onSelectNode={vi.fn()}
      onDrop={vi.fn()}
    />,
  );

  expect(view.container.querySelector(".react-flow")).not.toBeNull();
  expect(warning).not.toHaveBeenCalled();
  expect(error).not.toHaveBeenCalled();
});

it("does not publish another readiness state for an unchanged ResizeObserver measurement", () => {
  const callbacks: ResizeObserverCallback[] = [];
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        callbacks.push(callback);
      }
      observe() {}
      disconnect() {}
      unobserve() {}
    },
  );
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0, y: 0, top: 0, left: 0, right: 800, bottom: 600,
    width: 800, height: 600, toJSON: () => ({}),
  });
  const frame = vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    callback(0);
    return 1;
  });

  render(
    <WorkflowCanvas
      nodes={[]}
      edges={[]}
      onNodesChange={vi.fn()}
      onEdgesChange={vi.fn()}
      onConnect={vi.fn()}
      onConnectStart={vi.fn()}
      onConnectEnd={vi.fn()}
      isValidConnection={() => true}
      onSelectNode={vi.fn()}
      onDrop={vi.fn()}
    />,
  );
  callbacks[0]?.([], {} as ResizeObserver);

  expect(frame).toHaveBeenCalledOnce();
});
