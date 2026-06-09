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

function useStoredState<T>(key: string, fallback: T | (() => T)) {
  const [value, setValue] = useState<T>(() => readJson(key, fallback));

  useEffect(() => {
    window.localStorage.setItem(key, JSON.stringify(value));
  }, [key, value]);

  return [value, setValue] as const;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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
  const [projects, setProjects] = useStoredState<Project[]>(KEY_PROJECTS, seedProjects);
  const [assets, setAssets] = useStoredState<Asset[]>(KEY_ASSETS, seedAssets);
  const [history, setHistory] = useStoredState<GenerationHistory[]>(KEY_HISTORY, seedHistory);
  const [tasks, setTasks] = useStoredState<TaskRecord[]>(KEY_TASKS, seedTasks);
  const [batches, setBatches] = useStoredState<DerivedBatch[]>(KEY_BATCHES, seedBatches);
  const [rawConfig, setRawConfig] = useStoredState<AppConfig>(KEY_CONFIG, defaultConfig);
  const [ui, setUi] = useStoredState<AppUi>(KEY_UI, defaultUi);
  const config = useMemo(() => normalizeConfig(rawConfig), [rawConfig]);

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
