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
import type {
  AppConfig,
  AppUi,
  Asset,
  DerivedBatch,
  Folder,
  GenerationHistory,
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
  const source = isRecord(value) ? (value as Partial<AppConfig>) : {};
  const openaiPatch = isRecord(source.openai) ? (source.openai as Partial<AppConfig["openai"]>) : {};
  const seedancePatch = isRecord(source.seedance) ? (source.seedance as Partial<AppConfig["seedance"]>) : {};
  const openai = { ...defaultConfig.openai, ...openaiPatch };
  const seedance = { ...defaultConfig.seedance, ...seedancePatch };
  const resolutionOptions: AppConfig["seedance"]["resolution"][] = ["480P", "720P", "1080P"];
  const durationOptions: AppConfig["seedance"]["duration"][] = [3, 5, 6, 10];

  return {
    ...defaultConfig,
    ...source,
    openai: {
      apiKey: typeof openai.apiKey === "string" ? openai.apiKey : defaultConfig.openai.apiKey,
      baseUrl: typeof openai.baseUrl === "string" ? openai.baseUrl : defaultConfig.openai.baseUrl,
      textModel: typeof openai.textModel === "string" ? openai.textModel : defaultConfig.openai.textModel,
      imageModel:
        openai.imageModel === "gpt-image-1" || typeof openai.imageModel !== "string"
          ? defaultConfig.openai.imageModel
          : openai.imageModel,
      enabled: typeof openai.enabled === "boolean" ? openai.enabled : defaultConfig.openai.enabled
    },
    seedance: {
      enabled: typeof seedance.enabled === "boolean" ? seedance.enabled : defaultConfig.seedance.enabled,
      videoModel:
        typeof seedance.videoModel === "string" ? seedance.videoModel : defaultConfig.seedance.videoModel,
      audioModel:
        typeof seedance.audioModel === "string" ? seedance.audioModel : defaultConfig.seedance.audioModel,
      resolution: resolutionOptions.includes(seedance.resolution)
        ? seedance.resolution
        : defaultConfig.seedance.resolution,
      duration: durationOptions.includes(seedance.duration)
        ? seedance.duration
        : defaultConfig.seedance.duration,
      mockLatencyMs:
        typeof seedance.mockLatencyMs === "number" && Number.isFinite(seedance.mockLatencyMs)
          ? seedance.mockLatencyMs
          : defaultConfig.seedance.mockLatencyMs
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
