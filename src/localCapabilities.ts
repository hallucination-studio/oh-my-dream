import type {
  Asset,
  AssetRepository,
  GenerationTaskRunner,
  Project,
  ProjectRepository,
  SecretRef,
  SecretStore,
  TaskRecord
} from "./types";

export interface LocalCapabilityPorts {
  projects: ProjectRepository;
  assets: AssetRepository;
  secrets: SecretStore;
  tasks: GenerationTaskRunner;
}

function secretKey(ref: SecretRef) {
  return `omd.secret.${ref.provider}.${ref.key}`;
}

export function createBrowserLocalCapabilityPorts({
  getProjects,
  saveProject,
  deleteProject,
  getAssets,
  saveAsset,
  deleteAsset,
  enqueueTask,
  updateTask
}: {
  getProjects: () => Project[];
  saveProject: (project: Project) => void;
  deleteProject: (id: string) => void;
  getAssets: () => Asset[];
  saveAsset: (asset: Asset) => void;
  deleteAsset: (id: string) => void;
  enqueueTask: (task: TaskRecord) => void;
  updateTask: (taskId: string, patch: Partial<TaskRecord>) => TaskRecord | undefined;
}): LocalCapabilityPorts {
  return {
    projects: {
      listProjects: async () => getProjects(),
      saveProject: async (project) => {
        saveProject(project);
        return project;
      },
      deleteProject: async (id) => deleteProject(id)
    },
    assets: {
      listAssets: async () => getAssets(),
      saveAsset: async (asset) => {
        saveAsset(asset);
        return asset;
      },
      deleteAsset: async (id) => deleteAsset(id)
    },
    secrets: {
      getSecret: async (ref) => window.localStorage.getItem(secretKey(ref)) ?? undefined,
      setSecret: async (ref, value) => window.localStorage.setItem(secretKey(ref), value),
      clearSecret: async (ref) => window.localStorage.removeItem(secretKey(ref))
    },
    tasks: {
      enqueue: async (task) => {
        enqueueTask(task);
        return task;
      },
      cancel: async (taskId) => {
        updateTask(taskId, { status: "canceled", progress: 100 });
      },
      retry: async (taskId) => {
        const task = updateTask(taskId, { status: "queued", progress: 0 });
        if (!task) {
          throw new Error("Task not found");
        }
        return task;
      }
    }
  };
}
