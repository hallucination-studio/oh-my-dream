// Chooses the backend implementation at runtime: the real Tauri client when
// running inside a Tauri window, otherwise the in-browser mock. This is the
// single switch point — the App imports `api` and nothing else.

import type { WorkflowApi } from "./types.ts";
import { mockApi } from "./mockApi.ts";
import { tauriApi } from "./tauriApi.ts";

function isTauri(): boolean {
  // Tauri injects this global into the window it hosts.
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const api: WorkflowApi = isTauri() ? tauriApi : mockApi;

export type { WorkflowApi } from "./types.ts";
export type {
  Asset,
  AssetKind,
  AssetSort,
  ListAssetsOptions,
  Project,
  ProjectWorkspace,
  Provider,
  RunHandle,
  RunObserver,
} from "./types.ts";
