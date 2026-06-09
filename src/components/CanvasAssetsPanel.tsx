import { Upload } from "lucide-react";
import { useState } from "react";
import { formatGenerationParams } from "../services/generation";
import type { Asset, AssetCategory } from "../types";
import { MediaThumb } from "./CanvasMediaThumb";
import { Button } from "./ui";

const categories: { id: AssetCategory; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "other", label: "其它" },
  { id: "character", label: "人物" },
  { id: "scene", label: "场景" },
  { id: "object", label: "物品" },
  { id: "style", label: "风格" },
  { id: "sound", label: "音效" },
  { id: "project", label: "项目空间" }
];

export function AssetsPanel({
  assets,
  onUpload,
  onImport
}: {
  assets: Asset[];
  onUpload: (files: FileList | File[]) => void;
  onImport: (asset: Asset) => void;
}) {
  const [tab, setTab] = useState<"assets" | "subjects">("assets");
  const [category, setCategory] = useState<AssetCategory>("all");
  const filtered = assets.filter((asset) => category === "all" || asset.category === category);

  return (
    <div className="drawer-body">
      <div className="tab-row compact">
        <button type="button" className={tab === "assets" ? "active" : ""} onClick={() => setTab("assets")}>
          我的素材
        </button>
        <button type="button" className={tab === "subjects" ? "active" : ""} onClick={() => setTab("subjects")}>
          我的主体库
        </button>
      </div>
      <div className="chip-row">
        {categories.map((item) => (
          <button
            key={item.id}
            type="button"
            className={item.id === category ? "active" : ""}
            onClick={() => setCategory(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <label className="upload-zone small">
        <Upload size={16} />
        <span>上传素材</span>
        <input
          name="assetUpload"
          type="file"
          multiple
          accept="image/*,video/*,audio/*"
          onChange={(event) => event.target.files && onUpload(event.target.files)}
        />
      </label>
      {filtered.length === 0 ? (
        <p className="empty-copy">暂无素材。</p>
      ) : (
        <div className="asset-grid">
          {filtered.map((asset) => (
            <article className="asset-card" key={asset.id}>
              <MediaThumb kind={asset.kind} url={asset.url} />
              <strong>{asset.name}</strong>
              <span className="asset-source">{asset.resource.localPath ?? "本地缓存工作区"}</span>
              {asset.model && (
                <span className="asset-source">
                  {asset.provider ?? "local"} · {asset.model}
                </span>
              )}
              {asset.params && <p className="asset-params">{formatGenerationParams(asset.params)}</p>}
              {asset.prompt && <p className="asset-prompt">{asset.prompt}</p>}
              <Button size="sm" onClick={() => onImport(asset)}>
                插入画布
              </Button>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
