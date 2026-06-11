import { Save, Upload } from "lucide-react";
import { useCallback, useRef, useState, type ChangeEvent } from "react";
import { nowIso } from "../fixtures";
import { useStore } from "../storage";
import type { AppConfig, AppUi, Asset, DerivedBatch, GenerationHistory, Project, TaskRecord } from "../types";
import { Button, Modal } from "./ui";

interface WorkspaceBackup {
  version: 1;
  exportedAt: string;
  projects: Project[];
  assets: Asset[];
  history: GenerationHistory[];
  tasks: TaskRecord[];
  batches: DerivedBatch[];
  config: AppConfig;
  ui: AppUi;
}

function isWorkspaceBackup(value: unknown): value is WorkspaceBackup {
  const backup = value as Partial<WorkspaceBackup> | null;
  return Boolean(
    backup &&
      backup.version === 1 &&
      Array.isArray(backup.projects) &&
      Array.isArray(backup.assets) &&
      Array.isArray(backup.history) &&
      (!("tasks" in backup) || Array.isArray(backup.tasks)) &&
      (!("batches" in backup) || Array.isArray(backup.batches)) &&
      backup.config &&
      backup.ui
  );
}

export function WorkspaceStatusModal({ onClose }: { onClose: () => void }) {
  const {
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
    storageSchemaVersion
  } = useStore();
  const [backupMessage, setBackupMessage] = useState("");
  const importInputRef = useRef<HTMLInputElement | null>(null);

  const exportWorkspace = useCallback(() => {
    const backup: WorkspaceBackup = {
      version: 1,
      exportedAt: nowIso(),
      projects,
      assets,
      history,
      tasks,
      batches,
      config: {
        ...config,
        providers: {
          ...config.providers,
          openai: { ...config.providers.openai, apiKey: "" },
          volcengineArk: { ...config.providers.volcengineArk, apiKey: "" }
        }
      },
      ui
    };
    const blob = new Blob([JSON.stringify(backup, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `workspace-backup-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
    setBackupMessage("已导出工作区备份，OpenAI 与火山 Ark Key 未写入文件。");
  }, [assets, batches, config, history, projects, tasks, ui]);

  const importWorkspace = useCallback(
    async (event: ChangeEvent<HTMLInputElement>) => {
      const [file] = Array.from(event.target.files ?? []);
      event.target.value = "";
      if (!file) {
        return;
      }
      try {
        const parsed = JSON.parse(await file.text()) as unknown;
        if (!isWorkspaceBackup(parsed)) {
          throw new Error("备份格式不正确");
        }
        setProjects(parsed.projects);
        setAssets(parsed.assets);
        setHistory(parsed.history);
        setTasks(parsed.tasks ?? []);
        setBatches(parsed.batches ?? []);
        setConfig({
          ...parsed.config,
          providers: {
            ...parsed.config.providers,
            openai: {
              ...parsed.config.providers.openai,
              apiKey: parsed.config.providers.openai.apiKey || config.providers.openai.apiKey
            },
            volcengineArk: {
              ...parsed.config.providers.volcengineArk,
              apiKey: parsed.config.providers.volcengineArk.apiKey || config.providers.volcengineArk.apiKey
            }
          }
        });
        setUi(parsed.ui);
        setBackupMessage(`已导入 ${parsed.projects.length} 个项目、${parsed.assets.length} 个素材、${parsed.tasks?.length ?? 0} 个任务。`);
      } catch (error) {
        const message = error instanceof Error ? error.message : "导入失败";
        setBackupMessage(message);
      }
    },
    [
      config.providers.openai.apiKey,
      config.providers.volcengineArk.apiKey,
      setAssets,
      setBatches,
      setConfig,
      setHistory,
      setProjects,
      setTasks,
      setUi
    ]
  );

  return (
    <Modal title="本地状态" onClose={onClose} width={420}>
      <div className="status-grid">
        <article>
          <span>项目</span>
          <strong>{projects.length}</strong>
        </article>
        <article>
          <span>素材</span>
          <strong>{assets.length}</strong>
        </article>
        <article>
          <span>历史</span>
          <strong>{history.length}</strong>
        </article>
        <article>
          <span>任务</span>
          <strong>{tasks.length}</strong>
        </article>
        <article>
          <span>OpenAI</span>
          <strong>{config.providers.openai.enabled && config.providers.openai.apiKey ? "已配置" : "未配置"}</strong>
        </article>
        <article>
          <span>火山 Ark</span>
          <strong>{config.providers.volcengineArk.enabled && config.providers.volcengineArk.apiKey ? "已配置" : "未配置"}</strong>
        </article>
        <article>
          <span>Schema</span>
          <strong>v{storageSchemaVersion}</strong>
        </article>
        <article>
          <span>存储适配器</span>
          <strong>Browser local</strong>
        </article>
      </div>
      <div className="status-actions">
        <Button onClick={exportWorkspace}>
          <Save size={15} />
          导出工作区
        </Button>
        <Button onClick={() => importInputRef.current?.click()}>
          <Upload size={15} />
          导入备份
        </Button>
        <input
          ref={importInputRef}
          name="workspaceBackup"
          type="file"
          accept="application/json"
          hidden
          onChange={importWorkspace}
        />
      </div>
      <p className="status-note">
        当前使用浏览器本地存储作为桌面工作区的过渡适配器。备份包含项目、素材索引、历史、任务、配置与 UI 状态；API Key 默认不导出。
      </p>
      {backupMessage && <p className="status-message">{backupMessage}</p>}
    </Modal>
  );
}
