import { Check } from "lucide-react";
import { mockProvidersEnabled } from "../env";
import { useStore } from "../storage";
import { Button, Modal } from "./ui";

export function ConfigModal({ onClose }: { onClose: () => void }) {
  const { config, setConfig } = useStore();
  const { openai, volcengineArk, seedanceMock } = config.providers;
  return (
    <Modal title="系统配置" onClose={onClose} width={860}>
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
                checked={openai.enabled}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    providers: {
                      ...value.providers,
                      openai: { ...value.providers.openai, enabled: event.target.checked }
                    }
                  }))
                }
              />
              <span>启用文本与图像生成</span>
            </label>
            <TextField
              id="config-openai-api-key"
              label="API Key"
              type="password"
              value={openai.apiKey}
              onChange={(apiKey) =>
                setConfig((value) => ({
                  ...value,
                  providers: { ...value.providers, openai: { ...value.providers.openai, apiKey } }
                }))
              }
            />
            <TextField
              id="config-openai-base-url"
              label="Base URL"
              value={openai.baseUrl}
              onChange={(baseUrl) =>
                setConfig((value) => ({
                  ...value,
                  providers: { ...value.providers, openai: { ...value.providers.openai, baseUrl } }
                }))
              }
            />
            <TextField
              id="config-openai-text-model"
              label="文本模型"
              value={openai.models.text}
              onChange={(text) =>
                setConfig((value) => ({
                  ...value,
                  providers: {
                    ...value.providers,
                    openai: {
                      ...value.providers.openai,
                      models: { ...value.providers.openai.models, text }
                    }
                  }
                }))
              }
            />
            <TextField
              id="config-openai-image-model"
              label="图像模型"
              value={openai.models.image}
              onChange={(image) =>
                setConfig((value) => ({
                  ...value,
                  providers: {
                    ...value.providers,
                    openai: {
                      ...value.providers.openai,
                      models: { ...value.providers.openai.models, image }
                    }
                  }
                }))
              }
            />
          </section>

          <section>
            <h3>火山 Ark</h3>
            <label className="toggle-row" htmlFor="config-ark-enabled">
              <input
                id="config-ark-enabled"
                name="arkEnabled"
                type="checkbox"
                checked={volcengineArk.enabled}
                onChange={(event) =>
                  setConfig((value) => ({
                    ...value,
                    providers: {
                      ...value.providers,
                      volcengineArk: { ...value.providers.volcengineArk, enabled: event.target.checked }
                    }
                  }))
                }
              />
              <span>启用 Seedream / Seedance</span>
            </label>
            <TextField
              id="config-ark-api-key"
              label="API Key"
              type="password"
              value={volcengineArk.apiKey}
              onChange={(apiKey) =>
                setConfig((value) => ({
                  ...value,
                  providers: { ...value.providers, volcengineArk: { ...value.providers.volcengineArk, apiKey } }
                }))
              }
            />
            <TextField
              id="config-ark-base-url"
              label="Base URL"
              value={volcengineArk.baseUrl}
              onChange={(baseUrl) =>
                setConfig((value) => ({
                  ...value,
                  providers: { ...value.providers, volcengineArk: { ...value.providers.volcengineArk, baseUrl } }
                }))
              }
            />
            <TextField
              id="config-ark-image-model"
              label="图片模型"
              value={volcengineArk.models.image}
              onChange={(image) =>
                setConfig((value) => ({
                  ...value,
                  providers: {
                    ...value.providers,
                    volcengineArk: {
                      ...value.providers.volcengineArk,
                      models: { ...value.providers.volcengineArk.models, image }
                    }
                  }
                }))
              }
            />
            <TextField
              id="config-ark-video-model"
              label="视频模型"
              value={volcengineArk.models.video}
              onChange={(video) =>
                setConfig((value) => ({
                  ...value,
                  providers: {
                    ...value.providers,
                    volcengineArk: {
                      ...value.providers.volcengineArk,
                      models: { ...value.providers.volcengineArk.models, video }
                    }
                  }
                }))
              }
            />
            <div className="split-fields">
              <label htmlFor="config-ark-resolution">
                <span>视频分辨率</span>
                <select
                  id="config-ark-resolution"
                  value={volcengineArk.defaults.videoResolution}
                  onChange={(event) =>
                    setConfig((value) => ({
                      ...value,
                      providers: {
                        ...value.providers,
                        volcengineArk: {
                          ...value.providers.volcengineArk,
                          defaults: {
                            ...value.providers.volcengineArk.defaults,
                            videoResolution: event.target.value as typeof value.providers.volcengineArk.defaults.videoResolution
                          }
                        }
                      }
                    }))
                  }
                >
                  <option>480p</option>
                  <option>720p</option>
                  <option>1080p</option>
                </select>
              </label>
              <label htmlFor="config-ark-duration">
                <span>视频时长</span>
                <select
                  id="config-ark-duration"
                  value={volcengineArk.defaults.videoDuration}
                  onChange={(event) =>
                    setConfig((value) => ({
                      ...value,
                      providers: {
                        ...value.providers,
                        volcengineArk: {
                          ...value.providers.volcengineArk,
                          defaults: {
                            ...value.providers.volcengineArk.defaults,
                            videoDuration: Number(event.target.value) as typeof value.providers.volcengineArk.defaults.videoDuration
                          }
                        }
                      }
                    }))
                  }
                >
                  <option value={4}>4 秒</option>
                  <option value={5}>5 秒</option>
                  <option value={6}>6 秒</option>
                  <option value={8}>8 秒</option>
                  <option value={10}>10 秒</option>
                  <option value={12}>12 秒</option>
                  <option value={15}>15 秒</option>
                  <option value={-1}>智能时长</option>
                </select>
              </label>
            </div>
          </section>

          {mockProvidersEnabled && (
            <section>
              <h3>Seedance Mock</h3>
              <label className="toggle-row" htmlFor="config-mock-enabled">
                <input
                  id="config-mock-enabled"
                  name="mockEnabled"
                  type="checkbox"
                  checked={seedanceMock.enabled}
                  onChange={(event) =>
                    setConfig((value) => ({
                      ...value,
                      providers: {
                        ...value.providers,
                        seedanceMock: { ...value.providers.seedanceMock, enabled: event.target.checked }
                      }
                    }))
                  }
                />
                <span>启用调试 mock</span>
              </label>
              <TextField
                id="config-mock-video-model"
                label="视频模型"
                value={seedanceMock.models.video}
                onChange={(video) =>
                  setConfig((value) => ({
                    ...value,
                    providers: {
                      ...value.providers,
                      seedanceMock: {
                        ...value.providers.seedanceMock,
                        models: { ...value.providers.seedanceMock.models, video }
                      }
                    }
                  }))
                }
              />
              <TextField
                id="config-mock-latency"
                label="mock 延迟 ms"
                type="number"
                value={String(seedanceMock.mockLatencyMs)}
                onChange={(mockLatencyMs) =>
                  setConfig((value) => ({
                    ...value,
                    providers: {
                      ...value.providers,
                      seedanceMock: {
                        ...value.providers.seedanceMock,
                        mockLatencyMs: Math.min(10000, Math.max(300, Number(mockLatencyMs) || 300))
                      }
                    }
                  }))
                }
              />
            </section>
          )}
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

function TextField({
  id,
  label,
  value,
  type = "text",
  onChange
}: {
  id: string;
  label: string;
  value: string;
  type?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label htmlFor={id}>
      <span>{label}</span>
      <input
        id={id}
        name={id}
        type={type}
        autoComplete="off"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={type === "password" ? "只保存在本地客户端" : undefined}
      />
    </label>
  );
}
