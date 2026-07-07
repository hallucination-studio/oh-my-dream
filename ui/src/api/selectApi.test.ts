import { describe, expect, it } from "vitest";
import { api } from "./index.ts";
import { mockApi } from "./mockApi.ts";

describe("api selection", () => {
  it("uses mockApi when Tauri internals are absent", () => {
    expect("__TAURI_INTERNALS__" in window).toBe(false);
    expect(api).toBe(mockApi);
  });
});
