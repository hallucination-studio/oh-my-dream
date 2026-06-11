import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import {
  defaultConfig,
  defaultUi,
  KEY_ASSETS,
  KEY_BATCHES,
  KEY_CONFIG,
  KEY_HISTORY,
  KEY_PROJECTS,
  KEY_TASKS,
  KEY_UI,
  nowIso,
  seedAssets,
  seedBatches,
  seedHistory,
  seedProjects,
  seedTasks,
  uid
} from "./fixtures";
import { mockProvidersEnabled } from "./env";
import { createBrowserLocalCapabilityPorts, type LocalCapabilityPorts } from "./localCapabilities";
import type {
  AppConfig,
  AppUi,
  Asset,
  DerivedBatch,
  Folder,
  GenerationHistory,
  GenerationProvider,
  Project,
  TaskRecord
} from "./types";

type Setter<T> = React.Dispatch<React.SetStateAction<T>>;
const WORKSPACE_SCHEMA_VERSION = 1;

interface StoreContextValue {
  projects: Project[];
  setProjects: Setter<Project[]>;
  assets: Asset[];
  setAssets: Setter<Asset[]>;
  history: GenerationHistory[];
  setHistory: Setter<GenerationHistory[]>;
  tasks: TaskRecord[];
  setTasks: Setter<TaskRecord[]>;
  batches: DerivedBatch[];
  setBatches: Setter<DerivedBatch[]>;
  config: AppConfig;
  setConfig: Setter<AppConfig>;
  ui: AppUi;
  setUi: Setter<AppUi>;
  ports: LocalCapabilityPorts;
  storageSchemaVersion: number;
  updateProject: (id: string, patch: Partial<Project> | ((project: Project) => Project)) => void;
  duplicateProject: (id: string) => Project | undefined;
  deleteProject: (id: string) => void;
  createFolder: (name: string) => Folder;
}

const StoreContext = createContext<StoreContextValue | null>(null);

function readJson<T>(key: string, fallback: T | (() => T)): T {
  if (typeof window === "undefined") {
    return typeof fallback === "function" ? (fallback as () => T)() : fallback;
  }
  const raw = window.localStorage.getItem(key);
  if (!raw) {
    return typeof fallback === "function" ? (fallback as () => T)() : fallback;
  }
  try {
    return JSON.parse(raw) as T;
  } catch {
    return typeof fallback === "function" ? (fallback as () => T)() : fallback;
  }
}

function useStoredState<T>(key: string, fallback: T | (() => T), normalize?: (value: unknown) => T) {
  const [value, setValue] = useState<T>(() => {
    const parsed = readJson<unknown>(key, fallback);
    return normalize ? normalize(parsed) : (parsed as T);
  });

  useEffect(() => {
    const next = normalize ? normalize(value) : value;
    window.localStorage.setItem(key, JSON.stringify(next));
  }, [key, normalize, value]);

  return [value, setValue] as const;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeArray<T>(value: unknown, fallback: T[] | (() => T[])): T[] {
  if (Array.isArray(value)) {
    return value as T[];
  }
  return typeof fallback === "function" ? (fallback as () => T[])() : fallback;
}

function normalizeProject(value: Project): Project {
  const workspacePath = value.workspacePath || `workspace/${value.id}`;
  return {
    ...value,
    nodes: Array.isArray(value.nodes) ? value.nodes : [],
    edges: Array.isArray(value.edges) ? value.edges : [],
    createdAt: value.createdAt || nowIso(),
    updatedAt: value.updatedAt || value.createdAt || nowIso(),
    workspacePath,
    exportPath: value.exportPath || `${workspacePath}/exports`
  };
}

function normalizeUi(value: unknown): AppUi {
  const source = isRecord(value) ? value : {};
  return {
    ...defaultUi,
    ...source,
    folders: Array.isArray(source.folders) ? (source.folders as AppUi["folders"]) : defaultUi.folders
  };
}

function normalizeTask(value: TaskRecord): TaskRecord {
  const statusMap: Record<string, TaskRecord["status"]> = {
    pending: "queued",
    submitted: "queued",
    processing: "running",
    succeeded: "done",
    success: "done",
    error: "failed"
  };
  const rawStatus = String(value.status ?? "queued");
  const status = ["queued", "running", "done", "failed", "canceled"].includes(rawStatus)
    ? (rawStatus as TaskRecord["status"])
    : statusMap[rawStatus] ?? "queued";
  return {
    ...value,
    id: value.id || uid("task"),
    kind: value.kind || "generate",
    provider: value.provider || "local",
    title: value.title || value.detail || "生成任务",
    status,
    progress: Number.isFinite(value.progress) ? value.progress : status === "done" || status === "failed" ? 100 : 0,
    createdAt: value.createdAt || nowIso(),
    updatedAt: value.updatedAt || value.createdAt || nowIso()
  };
}

function normalizeConfig(value: unknown): AppConfig {
  const source = isRecord(value) ? value : {};
  const providers = isRecord(source.providers) ? source.providers : {};
  const openaiPatch = isRecord(providers.openai) ? providers.openai : {};
  const arkPatch = isRecord(providers.volcengineArk) ? providers.volcengineArk : {};
  const mockPatch = isRecord(providers.seedanceMock) ? providers.seedanceMock : {};
  const openaiModels = isRecord(openaiPatch.models) ? openaiPatch.models : {};
  const arkModels = isRecord(arkPatch.models) ? arkPatch.models : {};
  const arkDefaults = isRecord(arkPatch.defaults) ? arkPatch.defaults : {};
  const mockModels = isRecord(mockPatch.models) ? mockPatch.models : {};
  const mockDefaults = isRecord(mockPatch.defaults) ? mockPatch.defaults : {};
  const capabilityDefaults = isRecord(source.capabilityDefaults) ? source.capabilityDefaults : {};
  const mockResolutionOptions: AppConfig["providers"]["seedanceMock"]["defaults"]["resolution"][] = ["480P", "720P", "1080P"];
  const mockDurationOptions: AppConfig["providers"]["seedanceMock"]["defaults"]["duration"][] = [3, 5, 6, 10];
  const arkResolutionOptions: AppConfig["providers"]["volcengineArk"]["defaults"]["videoResolution"][] = ["480p", "720p", "1080p"];
  const arkRatioOptions: AppConfig["providers"]["volcengineArk"]["defaults"]["videoRatio"][] = [
    "adaptive",
    "16:9",
    "4:3",
    "1:1",
    "3:4",
    "9:16",
    "21:9"
  ];
  const arkDurationOptions: AppConfig["providers"]["volcengineArk"]["defaults"]["videoDuration"][] = [
    -1,
    4,
    5,
    6,
    8,
    10,
    12,
    15
  ];
  const providerOptions: GenerationProvider[] = mockProvidersEnabled
    ? ["openai", "volcengine-ark", "seedance-mock", "local"]
    : ["openai", "volcengine-ark", "local"];
  const providerOrDefault = (
    value: unknown,
    fallback: AppConfig["capabilityDefaults"][keyof AppConfig["capabilityDefaults"]]
  ): GenerationProvider => (providerOptions.includes(value as GenerationProvider) ? (value as GenerationProvider) : fallback);

  return {
    providers: {
      openai: {
        apiKey: typeof openaiPatch.apiKey === "string" ? openaiPatch.apiKey : defaultConfig.providers.openai.apiKey,
        baseUrl: typeof openaiPatch.baseUrl === "string" ? openaiPatch.baseUrl : defaultConfig.providers.openai.baseUrl,
        enabled: typeof openaiPatch.enabled === "boolean" ? openaiPatch.enabled : defaultConfig.providers.openai.enabled,
        models: {
          text: typeof openaiModels.text === "string" ? openaiModels.text : defaultConfig.providers.openai.models.text,
          image: typeof openaiModels.image === "string" ? openaiModels.image : defaultConfig.providers.openai.models.image
        }
      },
      volcengineArk: {
        apiKey:
          typeof arkPatch.apiKey === "string" ? arkPatch.apiKey : defaultConfig.providers.volcengineArk.apiKey,
        baseUrl:
          typeof arkPatch.baseUrl === "string" ? arkPatch.baseUrl : defaultConfig.providers.volcengineArk.baseUrl,
        enabled:
          typeof arkPatch.enabled === "boolean" ? arkPatch.enabled : defaultConfig.providers.volcengineArk.enabled,
        models: {
          image:
            typeof arkModels.image === "string"
              ? arkModels.image
              : defaultConfig.providers.volcengineArk.models.image,
          video:
            typeof arkModels.video === "string"
              ? arkModels.video
              : defaultConfig.providers.volcengineArk.models.video
        },
        defaults: {
          imageSize:
            typeof arkDefaults.imageSize === "string"
              ? arkDefaults.imageSize
              : defaultConfig.providers.volcengineArk.defaults.imageSize,
          videoResolution: arkResolutionOptions.includes(
            arkDefaults.videoResolution as AppConfig["providers"]["volcengineArk"]["defaults"]["videoResolution"]
          )
            ? (arkDefaults.videoResolution as AppConfig["providers"]["volcengineArk"]["defaults"]["videoResolution"])
            : defaultConfig.providers.volcengineArk.defaults.videoResolution,
          videoRatio: arkRatioOptions.includes(
            arkDefaults.videoRatio as AppConfig["providers"]["volcengineArk"]["defaults"]["videoRatio"]
          )
            ? (arkDefaults.videoRatio as AppConfig["providers"]["volcengineArk"]["defaults"]["videoRatio"])
            : defaultConfig.providers.volcengineArk.defaults.videoRatio,
          videoDuration: arkDurationOptions.includes(
            arkDefaults.videoDuration as AppConfig["providers"]["volcengineArk"]["defaults"]["videoDuration"]
          )
            ? (arkDefaults.videoDuration as AppConfig["providers"]["volcengineArk"]["defaults"]["videoDuration"])
            : defaultConfig.providers.volcengineArk.defaults.videoDuration,
          generateAudio:
            typeof arkDefaults.generateAudio === "boolean"
              ? arkDefaults.generateAudio
              : defaultConfig.providers.volcengineArk.defaults.generateAudio,
          watermark:
            typeof arkDefaults.watermark === "boolean"
              ? arkDefaults.watermark
              : defaultConfig.providers.volcengineArk.defaults.watermark
        }
      },
      seedanceMock: {
        enabled:
          mockProvidersEnabled && typeof mockPatch.enabled === "boolean"
            ? mockPatch.enabled
            : defaultConfig.providers.seedanceMock.enabled,
        models: {
          video:
            typeof mockModels.video === "string"
              ? mockModels.video
              : defaultConfig.providers.seedanceMock.models.video,
          audio:
            typeof mockModels.audio === "string"
              ? mockModels.audio
              : defaultConfig.providers.seedanceMock.models.audio
        },
        defaults: {
          resolution: mockResolutionOptions.includes(
            mockDefaults.resolution as AppConfig["providers"]["seedanceMock"]["defaults"]["resolution"]
          )
            ? (mockDefaults.resolution as AppConfig["providers"]["seedanceMock"]["defaults"]["resolution"])
            : defaultConfig.providers.seedanceMock.defaults.resolution,
          duration: mockDurationOptions.includes(
            mockDefaults.duration as AppConfig["providers"]["seedanceMock"]["defaults"]["duration"]
          )
            ? (mockDefaults.duration as AppConfig["providers"]["seedanceMock"]["defaults"]["duration"])
            : defaultConfig.providers.seedanceMock.defaults.duration
        },
        mockLatencyMs:
          typeof mockPatch.mockLatencyMs === "number" && Number.isFinite(mockPatch.mockLatencyMs)
            ? mockPatch.mockLatencyMs
            : defaultConfig.providers.seedanceMock.mockLatencyMs
      }
    },
    capabilityDefaults: {
      text: providerOrDefault(capabilityDefaults.text, defaultConfig.capabilityDefaults.text),
      image: providerOrDefault(capabilityDefaults.image, defaultConfig.capabilityDefaults.image),
      video: providerOrDefault(capabilityDefaults.video, defaultConfig.capabilityDefaults.video),
      audio: providerOrDefault(capabilityDefaults.audio, defaultConfig.capabilityDefaults.audio)
    }
  };
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [projects, setProjectsRaw] = useStoredState<Project[]>(
    KEY_PROJECTS,
    seedProjects,
    (value) => normalizeArray<Project>(value, seedProjects).map(normalizeProject)
  );
  const [assets, setAssets] = useStoredState<Asset[]>(KEY_ASSETS, seedAssets, (value) =>
    normalizeArray<Asset>(value, seedAssets)
  );
  const [history, setHistory] = useStoredState<GenerationHistory[]>(KEY_HISTORY, seedHistory, (value) =>
    normalizeArray<GenerationHistory>(value, seedHistory)
  );
  const [tasks, setTasks] = useStoredState<TaskRecord[]>(KEY_TASKS, seedTasks, (value) =>
    normalizeArray<TaskRecord>(value, seedTasks).map(normalizeTask)
  );
  const [batches, setBatches] = useStoredState<DerivedBatch[]>(KEY_BATCHES, seedBatches, (value) =>
    normalizeArray<DerivedBatch>(value, seedBatches)
  );
  const [rawConfig, setRawConfig] = useStoredState<AppConfig>(KEY_CONFIG, defaultConfig, normalizeConfig);
  const [uiRaw, setUiRaw] = useStoredState<AppUi>(KEY_UI, defaultUi, normalizeUi);
  const ui = useMemo(() => normalizeUi(uiRaw), [uiRaw]);
  const config = useMemo(() => normalizeConfig(rawConfig), [rawConfig]);

  const setProjects = useCallback<Setter<Project[]>>(
    (next) => {
      setProjectsRaw((current) => {
        const normalized = current.map(normalizeProject);
        const value = typeof next === "function" ? (next as (value: Project[]) => Project[])(normalized) : next;
        return normalizeArray<Project>(value, []).map(normalizeProject);
      });
    },
    [setProjectsRaw]
  );

  const setUi = useCallback<Setter<AppUi>>(
    (next) => {
      setUiRaw((current) => {
        const normalized = normalizeUi(current);
        const value = typeof next === "function" ? (next as (value: AppUi) => AppUi)(normalized) : next;
        return normalizeUi(value);
      });
    },
    [setUiRaw]
  );

  const setConfig = useCallback<Setter<AppConfig>>(
    (next) => {
      setRawConfig((current) => {
        const normalized = normalizeConfig(current);
        const value = typeof next === "function" ? (next as (value: AppConfig) => AppConfig)(normalized) : next;
        return normalizeConfig(value);
      });
    },
    [setRawConfig]
  );

  useEffect(() => {
    const normalized = normalizeConfig(rawConfig);
    if (JSON.stringify(normalized) !== JSON.stringify(rawConfig)) {
      setRawConfig(normalized);
    }
  }, [rawConfig, setRawConfig]);

  useEffect(() => {
    const normalized = projects.map(normalizeProject);
    if (JSON.stringify(normalized) !== JSON.stringify(projects)) {
      setProjectsRaw(normalized);
    }
  }, [projects, setProjectsRaw]);

  useEffect(() => {
    const normalized = normalizeUi(uiRaw);
    if (JSON.stringify(normalized) !== JSON.stringify(uiRaw)) {
      setUiRaw(normalized);
    }
  }, [setUiRaw, uiRaw]);

  const updateProject = useCallback<StoreContextValue["updateProject"]>((id, patch) => {
    setProjects((items) =>
      items.map((project) => {
        if (project.id !== id) {
          return project;
        }
        const next =
          typeof patch === "function"
            ? patch(project)
            : { ...project, ...patch, updatedAt: patch.updatedAt ?? nowIso() };
        return { ...next, updatedAt: next.updatedAt ?? nowIso() };
      })
    );
  }, [setProjects]);

  const duplicateProject = useCallback<StoreContextValue["duplicateProject"]>((id) => {
    let copy: Project | undefined;
    setProjects((items) => {
      const source = items.find((item) => item.id === id);
      if (!source) {
        return items;
      }
      const createdAt = nowIso();
      copy = {
        ...source,
        id: uid("project"),
        name: `${source.name} 副本`,
        createdAt,
        updatedAt: createdAt,
        readonly: false,
        nodes: source.nodes.map((node) => ({
          ...node,
          id: uid(node.data.kind),
          selected: false,
          data: { ...node.data, readonly: false },
          position: { x: node.position.x + 48, y: node.position.y + 48 }
        })),
        edges: []
      };
      const idMap = new Map(source.nodes.map((node, index) => [node.id, copy!.nodes[index].id]));
      copy.edges = source.edges
        .filter((edge) => idMap.has(edge.source) && idMap.has(edge.target))
        .map((edge) => ({
          ...edge,
          id: uid("edge"),
          source: idMap.get(edge.source)!,
          target: idMap.get(edge.target)!
        }));
      return [copy, ...items];
    });
    return copy;
  }, [setProjects]);

  const deleteProject = useCallback<StoreContextValue["deleteProject"]>((id) => {
    setProjects((items) => items.filter((project) => project.id !== id));
  }, [setProjects]);

  const createFolder = useCallback<StoreContextValue["createFolder"]>((name) => {
    const folder: Folder = { id: uid("folder"), name, createdAt: nowIso() };
    setUi((value) => ({ ...value, folders: [folder, ...value.folders] }));
    return folder;
  }, [setUi]);

  const ports = useMemo(
    () =>
      createBrowserLocalCapabilityPorts({
        getProjects: () => projects,
        saveProject: (project) => setProjects((items) => [normalizeProject(project), ...items.filter((item) => item.id !== project.id)]),
        deleteProject,
        getAssets: () => assets,
        saveAsset: (asset) => setAssets((items) => [asset, ...items.filter((item) => item.id !== asset.id)]),
        deleteAsset: (id) => setAssets((items) => items.filter((item) => item.id !== id)),
        enqueueTask: (task) => setTasks((items) => [task, ...items]),
        updateTask: (taskId, patch) => {
          let updated: TaskRecord | undefined;
          setTasks((items) =>
            items.map((task) => {
              if (task.id !== taskId) {
                return task;
              }
              updated = { ...task, ...patch, updatedAt: nowIso() };
              return updated;
            })
          );
          return updated;
        }
      }),
    [assets, deleteProject, projects, setAssets, setProjects, setTasks]
  );

  const value = useMemo<StoreContextValue>(
    () => ({
      projects,
      setProjects,
      assets,
      setAssets,
      history,
      setHistory,
      tasks,
      setTasks,
      batches,
      setBatches,
      config,
      setConfig,
      ui,
      setUi,
      ports,
      storageSchemaVersion: WORKSPACE_SCHEMA_VERSION,
      updateProject,
      duplicateProject,
      deleteProject,
      createFolder
    }),
    [
      projects,
      setProjects,
      assets,
      setAssets,
      history,
      setHistory,
      tasks,
      setTasks,
      batches,
      setBatches,
      config,
      setConfig,
      ui,
      setUi,
      ports,
      updateProject,
      duplicateProject,
      deleteProject,
      createFolder
    ]
  );

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore() {
  const value = useContext(StoreContext);
  if (!value) {
    throw new Error("useStore must be used inside StoreProvider");
  }
  return value;
}
