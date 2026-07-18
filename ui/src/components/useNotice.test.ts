import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useNotice } from "./useNotice.ts";

afterEach(() => vi.useRealTimers());

it("shows a transient notice and clears it after the timeout", () => {
  vi.useFakeTimers();
  const view = renderHook(() => useNotice(1000));

  act(() => view.result.current.notify("Connection rejected · the port types are incompatible"));
  expect(view.result.current.notice).toBe("Connection rejected · the port types are incompatible");

  act(() => view.result.current.notify("Second notice"));
  expect(view.result.current.notice).toBe("Second notice");

  act(() => vi.advanceTimersByTime(1100));
  expect(view.result.current.notice).toBeNull();
});
