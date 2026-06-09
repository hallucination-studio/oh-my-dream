import { Check } from "lucide-react";
import { useStore } from "../storage";
import { Button, Modal } from "./ui";

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
