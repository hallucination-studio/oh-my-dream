import { Archive, Check, Save, Settings, Sparkles, Upload, X } from "lucide-react";
import { useCallback, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { nowIso } from "../fixtures";
import { useStore } from "../storage";
import type { AppConfig, AppUi, Asset, GenerationHistory, Project } from "../types";
import { HelpPanel } from "./CanvasPanels";
import { Button, IconButton, Modal } from "./ui";

interface WorkspaceBackup {
  version: 1;
  exportedAt: string;
  projects: Project[];
  assets: Asset[];
  history: GenerationHistory[];
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
      backup.config &&
      backup.ui
  );
}

export function AppShell({ children }: { children: ReactNode }) {
  const {
    projects,
    setProjects,
    assets,
    setAssets,
    history,
    setHistory,
    config,
    setConfig,
    ui,
    setUi
  } = useStore();
  const [configOpen, setConfigOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [statusOpen, setStatusOpen] = useState(false);
  const [backupMessage, setBackupMessage] = useState("");
  const importInputRef = useRef<HTMLInputElement | null>(null);

  const exportWorkspace = useCallback(() => {
    const backup: WorkspaceBackup = {
      version: 1,
      exportedAt: nowIso(),
      projects,
      assets,
      history,
      config: {
        ...config,
        openai: { ...config.openai, apiKey: "" }
      },
      ui
    };
    const blob = new Blob([JSON.stringify(backup, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `libtv-local-workspace-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
    setBackupMessage("已导出工作区备份，OpenAI Key 未写入文件。");
  }, [assets, config, history, projects, ui]);

  const importWorkspace = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
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
        setConfig({
          ...parsed.config,
          openai: {
            ...parsed.config.openai,
            apiKey: parsed.config.openai.apiKey || config.openai.apiKey
          }
        });
        setUi(parsed.ui);
        setBackupMessage(`已导入 ${parsed.projects.length} 个项目、${parsed.assets.length} 个素材。`);
      } catch (error) {
        const message = error instanceof Error ? error.message : "导入失败";
        setBackupMessage(message);
      }
    },
    [config.openai.apiKey, setAssets, setConfig, setHistory, setProjects, setUi]
  );

  return (
    <div className="app-page app-shell">
      {!ui.bannerClosed && (
        <div className="activity-bar">
          <strong className="activity-pill">
            <Sparkles size={15} />
            本地创作
          </strong>
          <span>Seedance 2.0 mock 已接入 · OpenAI 文本与图像走本地配置 · 纯本地创作链路</span>
          <IconButton label="隐藏活动条" onClick={() => setUi((value) => ({ ...value, bannerClosed: true }))}>
            <X size={16} />
          </IconButton>
        </div>
      )}
      <header className="topbar">
        <Link to="/" className="brand" aria-label="返回首页">
          <span className="brand-mark">TV</span>
          <span>LibTV</span>
          <small>Local</small>
        </Link>
        <nav className="topbar-nav" aria-label="主导航">
          <Link to="/project">项目</Link>
          <button type="button" className="plain-link">创作者挑战赛</button>
          <button type="button" className="plain-link" onClick={() => setHelpOpen(true)}>
            本地帮助
          </button>
        </nav>
        <div className="topbar-actions">
          <IconButton label="系统配置" onClick={() => setConfigOpen(true)}>
            <Settings size={18} />
          </IconButton>
          <IconButton label="本地状态" onClick={() => setStatusOpen(true)}>
            <Archive size={18} />
          </IconButton>
        </div>
      </header>
      {children}
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
      {helpOpen && <HelpModal onClose={() => setHelpOpen(false)} />}
      {statusOpen && (
        <Modal title="本地状态" onClose={() => setStatusOpen(false)} width={420}>
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
              <span>OpenAI</span>
              <strong>{config.openai.enabled && config.openai.apiKey ? "已配置" : "未配置"}</strong>
            </article>
            <article>
              <span>Seedance</span>
              <strong>{config.seedance.enabled ? "Mock 开" : "Mock 关"}</strong>
            </article>
            <article>
              <span>存储</span>
              <strong>localStorage</strong>
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
          <p className="status-note">备份包含项目、素材、历史、配置与 UI 状态；OpenAI Key 默认不导出。</p>
          {backupMessage && <p className="status-message">{backupMessage}</p>}
        </Modal>
      )}
    </div>
  );
}

function HelpModal({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="帮助中心" onClose={onClose} width={560}>
      <HelpPanel />
    </Modal>
  );
}

export function ConfigModal({ onClose }: { onClose: () => void }) {
  const { config, setConfig } = useStore();
  return (
    <Modal title="系统配置" onClose={onClose} width={720}>
      <form
        className="config-form"
        onSubmit={(event) => {
          event.preventDefault();
          onClose();
        }}
      >
        <div className="config-grid">
          <section>
            <h3>OpenAI</h3>
            <label className="toggle-row" htmlFor="config-openai-enabled">
              <input
                id="config-openai-enabled"
                name="openaiEnabled"
                type="checkbox"
                checked={config.openai.enabled}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    openai: { ...value.openai, enabled: event.target.checked }
                  }))
                }
              />
              <span>启用文本与图像生成</span>
            </label>
            <label htmlFor="config-openai-api-key">
              <span>API Key</span>
              <input
                id="config-openai-api-key"
                name="openaiApiKey"
                type="password"
                autoComplete="off"
                value={config.openai.apiKey}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    openai: { ...value.openai, apiKey: event.target.value }
                  }))
                }
                placeholder="只保存在本地浏览器"
              />
            </label>
            <label htmlFor="config-openai-base-url">
              <span>Base URL</span>
              <input
                id="config-openai-base-url"
                name="openaiBaseUrl"
                value={config.openai.baseUrl}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    openai: { ...value.openai, baseUrl: event.target.value }
                  }))
                }
              />
            </label>
            <label htmlFor="config-openai-text-model">
              <span>文本模型</span>
              <input
                id="config-openai-text-model"
                name="openaiTextModel"
                value={config.openai.textModel}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    openai: { ...value.openai, textModel: event.target.value }
                  }))
                }
              />
            </label>
            <label htmlFor="config-openai-image-model">
              <span>图像模型</span>
              <input
                id="config-openai-image-model"
                name="openaiImageModel"
                value={config.openai.imageModel}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    openai: { ...value.openai, imageModel: event.target.value }
                  }))
                }
              />
            </label>
          </section>
          <section>
            <h3>Seedance Mock</h3>
            <label className="toggle-row" htmlFor="config-seedance-enabled">
              <input
                id="config-seedance-enabled"
                name="seedanceEnabled"
                type="checkbox"
                checked={config.seedance.enabled}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    seedance: { ...value.seedance, enabled: event.target.checked }
                  }))
                }
              />
              <span>启用 mock 生成</span>
            </label>
            <label htmlFor="config-seedance-video-model">
              <span>视频模型</span>
              <input
                id="config-seedance-video-model"
                name="seedanceVideoModel"
                value={config.seedance.videoModel}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    seedance: { ...value.seedance, videoModel: event.target.value }
                  }))
                }
              />
            </label>
            <label htmlFor="config-seedance-audio-model">
              <span>音频模型</span>
              <input
                id="config-seedance-audio-model"
                name="seedanceAudioModel"
                value={config.seedance.audioModel}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    seedance: { ...value.seedance, audioModel: event.target.value }
                  }))
                }
              />
            </label>
            <div className="split-fields">
              <label htmlFor="config-seedance-resolution">
                <span>分辨率</span>
                <select
                  id="config-seedance-resolution"
                  name="seedanceResolution"
                  value={config.seedance.resolution}
                  onChange={(event) =>
                    setConfig((value) => ({
                      ...value,
                      seedance: {
                        ...value.seedance,
                        resolution: event.target.value as typeof value.seedance.resolution
                      }
                    }))
                  }
                >
                  <option>480P</option>
                  <option>720P</option>
                  <option>1080P</option>
                </select>
              </label>
              <label htmlFor="config-seedance-duration">
                <span>时长</span>
                <select
                  id="config-seedance-duration"
                  name="seedanceDuration"
                  value={config.seedance.duration}
                  onChange={(event) =>
                    setConfig((value) => ({
                      ...value,
                      seedance: {
                        ...value.seedance,
                        duration: Number(event.target.value) as typeof value.seedance.duration
                      }
                    }))
                  }
                >
                  <option value={3}>3 秒</option>
                  <option value={5}>5 秒</option>
                  <option value={6}>6 秒</option>
                  <option value={10}>10 秒</option>
                </select>
              </label>
            </div>
            <label htmlFor="config-seedance-mock-latency">
              <span>mock 延迟 ms</span>
              <input
                id="config-seedance-mock-latency"
                name="seedanceMockLatency"
                type="number"
                min={300}
                step={100}
                value={config.seedance.mockLatencyMs}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    seedance: { ...value.seedance, mockLatencyMs: Number(event.target.value) }
                  }))
                }
              />
            </label>
          </section>
        </div>
        <div className="modal-actions">
          <Button type="submit" variant="primary">
            <Check size={16} />
            保存
          </Button>
        </div>
      </form>
    </Modal>
  );
}
