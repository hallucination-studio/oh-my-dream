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

export type ImageToolName =
  | "panorama"
  | "angles"
  | "lighting"
  | "grid"
  | "upscale"
  | "split"
  | "annotate"
  | "rotate";

export type TaskStatus = "queued" | "running" | "done" | "failed" | "canceled";
export type TaskKind = "generate" | "derive" | "download";

export interface LocalMediaResource {
  id: string;
  kind: AssetKind;
  title: string;
  mimeType?: string;
  localPath?: string;
  cachePath?: string;
  remoteUrl?: string;
  dataUrl?: string;
  width?: number;
  height?: number;
  fileSize?: number;
  createdAt: string;
}

export interface PreviewResource {
  id: string;
  title: string;
  kind: AssetKind;
  items: LocalMediaResource[];
  activeIndex?: number;
  sourceNodeId?: string;
}

export interface DownloadArtifact {
  id: string;
  name: string;
  kind: AssetKind;
  resourceId: string;
  fileName: string;
  targetPath?: string;
}

export interface DerivedBatch {
  id: string;
  tool: ImageToolName;
  sourceNodeId: string;
  sourceAssetId?: string;
  resultNodeIds: string[];
  resultAssetIds: string[];
  outputCount: number;
  createdAt: string;
}

export interface NodeReference {
  id: string;
  label: string;
  kind: "node" | "asset" | "history";
}

export interface NodeOutput {
  resources: LocalMediaResource[];
  preview?: PreviewResource;
  downloads?: DownloadArtifact[];
  batchId?: string;
}

export interface CanvasNodeData extends Record<string, unknown> {
  kind: NodeKind;
  name: string;
  prompt?: string;
  text?: string;
  url?: string;
  urls?: string[];
  params?: GenerationParams;
  taskInfo?: {
    status: TaskStatus;
    progress?: number;
    message?: string;
  };
  contentWidth?: number;
  contentHeight?: number;
  readonly?: boolean;
  workflowType?: "base" | "generated" | "derived" | "asset" | "reference";
  toolName?: ImageToolName;
  sourceRefs?: NodeReference[];
  output?: NodeOutput;
  annotations?: string[];
  localPath?: string;
  cachePath?: string;
  remoteUrl?: string;
}

export type LibNode = Node<CanvasNodeData, "libNode">;
export type LibEdge = Edge;

export interface LocalProject {
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
  workspacePath?: string;
  exportPath?: string;
}

export type Project = LocalProject;

export interface Folder {
  id: string;
  name: string;
  createdAt: string;
}

export interface AssetRecord {
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
  resource: LocalMediaResource;
  sourceNodeId?: string;
  batchId?: string;
  tags?: string[];
  uses?: number;
}

export type Asset = AssetRecord;

export interface GenerationHistory {
  id: string;
  kind: GenerationKind;
  provider: GenerationProvider;
  model: string;
  prompt: string;
  status: TaskStatus;
  progress: number;
  resultUrl?: string;
  resultText?: string;
  revisedPrompt?: string;
  params?: GenerationParams;
  error?: string;
  createdAt: string;
  resultResources?: LocalMediaResource[];
  sourceNodeId?: string;
  batchId?: string;
}

export interface TaskRecord {
  id: string;
  kind: TaskKind;
  status: TaskStatus;
  title: string;
  provider: GenerationProvider | "desktop";
  sourceNodeId?: string;
  batchId?: string;
  progress: number;
  detail?: string;
  artifacts?: DownloadArtifact[];
  createdAt: string;
  updatedAt: string;
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
