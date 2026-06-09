import { CircleHelp } from "lucide-react";
import { toolboxPresets } from "../fixtures";
import { Button } from "./ui";

export function ToolboxPanel({ onUse }: { onUse: (presetId: string) => void }) {
  return (
    <div className="drawer-body">
      <div className="toolbox-tabs">
        <button type="button" className="active">
          我的工具箱
        </button>
        <Button size="sm">
          <CircleHelp size={14} />
          模板说明
        </Button>
      </div>
      <div className="toolbox-grid">
        {toolboxPresets.map((preset) => (
          <article key={preset.id} className="tool-card">
            <img src={preset.thumb} alt="" loading="lazy" />
            <h3>{preset.name}</h3>
            <p>{preset.description}</p>
            <Button size="sm" variant="primary" onClick={() => onUse(preset.id)}>
              使用
            </Button>
          </article>
        ))}
      </div>
    </div>
  );
}
