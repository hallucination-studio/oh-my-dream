import type { Edge, Node, Viewport } from "@xyflow/react";

export type NodeKind =
  | "text"
  | "image"
  | "video"
  | "audio"
  | "compose"
  | "director"
  | "script"
  | "group";

export type AssetKind = "image" | "video" | "audio";
export type AssetCategory =
  | "all"
  | "other"
  | "character"
  | "scene"
  | "object"
  | "style"
  | "sound"
  | "project";

export type GenerationKind = AssetKind | "text";
export type GenerationProvider = "openai" | "seedance-mock" | "local";
export type GenerationParams = Record<string, string | number | boolean>;

export interface CanvasNodeData extends Record<string, unknown> {
  kind: NodeKind;
  name: string;
  prompt?: string;
  text?: string;
  url?: string;
  urls?: string[];
  params?: GenerationParams;
  taskInfo?: {
    status: "queued" | "running" | "done" | "failed";
    progress?: number;
    message?: string;
  };
  contentWidth?: number;
  contentHeight?: number;
  readonly?: boolean;
}

export type LibNode = Node<CanvasNodeData, "libNode">;
export type LibEdge = Edge;

export interface Project {
  id: string;
  name: string;
  coverUrl: string;
  createdAt: string;
  updatedAt: string;
  folderId?: string;
  nodes: LibNode[];
  edges: LibEdge[];
  viewport?: Viewport;
  readonly?: boolean;
}

export interface Folder {
  id: string;
  name: string;
  createdAt: string;
}

export interface Asset {
  id: string;
  kind: AssetKind;
  name: string;
  url: string;
  category: AssetCategory;
  provider?: GenerationProvider;
  model?: string;
  prompt?: string;
  params?: GenerationParams;
  createdAt: string;
}

export interface GenerationHistory {
  id: string;
  kind: GenerationKind;
  provider: GenerationProvider;
  model: string;
  prompt: string;
  status: "queued" | "running" | "done" | "failed";
  progress: number;
  resultUrl?: string;
  resultText?: string;
  revisedPrompt?: string;
  params?: GenerationParams;
  error?: string;
  createdAt: string;
}

export interface AppConfig {
  openai: {
    apiKey: string;
    baseUrl: string;
    textModel: string;
    imageModel: string;
    enabled: boolean;
  };
  seedance: {
    enabled: boolean;
    videoModel: string;
    audioModel: string;
    resolution: "480P" | "720P" | "1080P";
    duration: 3 | 5 | 6 | 10;
    mockLatencyMs: number;
  };
}

export interface AppUi {
  bannerClosed: boolean;
  minimap: boolean;
  snapToGrid: boolean;
  folders: Folder[];
}
