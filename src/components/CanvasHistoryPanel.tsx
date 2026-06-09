import { useState, type Dispatch, type SetStateAction } from "react";
import { formatGenerationParams, historyDisplayText } from "../services/generation";
import type { GenerationHistory } from "../types";
import { formatDate } from "../utils";
import { MediaThumb } from "./CanvasMediaThumb";
import { Button } from "./ui";

export function HistoryPanel({
  history,
  setHistory,
  onImport
}: {
  history: GenerationHistory[];
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  onImport: (item: GenerationHistory) => void;
}) {
  const [tab, setTab] = useState<GenerationHistory["kind"]>("text");
  const [size, setSize] = useState(92);
  const [selected, setSelected] = useState<string[]>([]);
  const items = history
    .filter((item) => item.kind === tab)
    .sort((a, b) => +new Date(b.createdAt) - +new Date(a.createdAt));

  const removeSelected = () => {
    setHistory((records) => records.filter((item) => !selected.includes(item.id)));
    setSelected([]);
  };
  const selectTab = (next: GenerationHistory["kind"]) => {
    setTab(next);
    setSelected([]);
  };

  return (
    <div className="drawer-body">
      <div className="history-head">
        <div className="tab-row compact">
          <button type="button" className={tab === "text" ? "active" : ""} onClick={() => selectTab("text")}>
            文本历史
          </button>
          <button type="button" className={tab === "image" ? "active" : ""} onClick={() => selectTab("image")}>
            图片历史
          </button>
          <button type="button" className={tab === "video" ? "active" : ""} onClick={() => selectTab("video")}>
            视频历史
          </button>
          <button type="button" className={tab === "audio" ? "active" : ""} onClick={() => selectTab("audio")}>
            音频历史
          </button>
        </div>
        <label className="range-control">
          <span>缩略图</span>
          <input
            name="historyThumbSize"
            type="range"
            min={68}
            max={142}
            value={size}
            onChange={(event) => setSize(Number(event.target.value))}
          />
        </label>
      </div>
      <div className="batch-row">
        <Button size="sm" onClick={() => setSelected(items.map((item) => item.id))}>
          全选
        </Button>
        <Button size="sm" variant="danger" disabled={selected.length === 0} onClick={removeSelected}>
          批量删除
        </Button>
      </div>
      {items.length === 0 ? (
        <p className="empty-copy">暂无历史记录。</p>
      ) : (
        <div className="history-list">
          {items.map((item) => (
            <article key={item.id} className="history-card" style={{ gridTemplateColumns: `${size}px 1fr` }}>
              <label className="check-cell">
                <input
                  type="checkbox"
                  checked={selected.includes(item.id)}
                  onChange={(event) =>
                    setSelected((values) =>
                      event.target.checked
                        ? [...values, item.id]
                        : values.filter((value) => value !== item.id)
                    )
                  }
                />
                <MediaThumb kind={item.kind} url={item.resultUrl} />
              </label>
              <div>
                <strong>{historyDisplayText(item)}</strong>
                {item.resultText && <p className="history-prompt">Prompt: {item.prompt}</p>}
                {item.revisedPrompt && <p className="history-prompt">修订 Prompt: {item.revisedPrompt}</p>}
                {item.params && <p className="history-prompt">参数: {formatGenerationParams(item.params)}</p>}
                {item.error && <p className="history-error">{item.error}</p>}
                <span>
                  {item.model} · {formatDate(item.createdAt)}
                </span>
                <em className={`history-status ${item.status}`}>
                  {item.provider} · {item.status} · {item.progress}%
                </em>
                <div className="history-actions">
                  <Button size="sm" onClick={() => onImport(item)}>
                    导入画布
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => setHistory((records) => records.filter((record) => record.id !== item.id))}
                  >
                    删除
                  </Button>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
