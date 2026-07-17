import type { AssetViewModel } from "./model.ts";

interface Props {
  asset: AssetViewModel;
  className: string;
  controls?: boolean;
}

/** Renders the signed preview URI through its exact media element. */
export function AssetMediaPreview({ asset, className, controls = false }: Props) {
  if (!asset.previewUrl) {
    return <span className={className}>{asset.kind === "audio" ? "♪" : asset.kind}</span>;
  }
  if (asset.kind === "image") {
    return <img className={className} src={asset.previewUrl} alt={asset.displayName} loading="lazy" />;
  }
  if (asset.kind === "video") {
    return (
      <video
        className={className}
        src={asset.previewUrl}
        aria-label={asset.displayName}
        controls={controls}
        muted={!controls}
        preload="metadata"
      />
    );
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
