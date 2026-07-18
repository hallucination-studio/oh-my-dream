import type { AssetViewModel } from "./model.ts";

interface Props {
  asset: AssetViewModel;
  className: string;
}

/** Renders the issued preview URI through its media element. */
export function AssetMediaPreview({ asset, className }: Props) {
  if (!asset.previewUrl) {
    return <span className={className}>{asset.kind === "audio" ? "♪" : asset.kind}</span>;
  }
  if (asset.kind === "image") {
    return <img className={className} src={asset.previewUrl} alt={asset.displayName} loading="lazy" />;
  }
  if (asset.kind === "video") {
    // Video previews are poster images until the preview-kind contract (G9)
    // declares playable files; posters never render through <video>.
    return <img className={className} src={asset.previewUrl} alt={asset.displayName} loading="lazy" />;
  }
  return (
    <audio
      className={className}
      src={asset.previewUrl}
      aria-label={asset.displayName}
      controls
      preload="metadata"
    />
  );
}
