// Right inspector: parameters of the selected node in a calm flush panel, plus
// a note that generated media auto-saves to the library.

import type { NodeTypeSpec } from "../nodes/catalog.ts";
import { ParameterInput } from "../nodes/ParameterInput.tsx";
import { GenerationProfileSelector } from "./GenerationProfileSelector.tsx";
import { nodeAccent } from "../nodes/typeColor.ts";
import "./inspector.css";

export interface SelectedNode {
  id: string;
  type: string;
  params: Record<string, unknown>;
  capability?: NodeTypeSpec;
  assetPresentation?: { title: string; available: boolean };
}

export function InspectorPanel({
  node,
  modeOptions = [],
  onModeChange = () => undefined,
  onParamChange,
  onOpenAssets = () => undefined,
}: {
  node: SelectedNode | null;
  modeOptions?: NodeTypeSpec[];
  onModeChange?: (mode: string) => void;
  onParamChange: (nodeId: string, name: string, value: unknown) => void;
  onOpenAssets?: () => void;
}) {
  if (!node) {
    return (
      <aside className="insp">
        <div className="insp__empty">
          <span className="insp__empty-mark" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
              <path d="M12 20h9" />
              <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z" />
            </svg>
          </span>
          <span>Select a node to edit its parameters.</span>
        </div>
      </aside>
    );
  }

  const spec = node.capability;
  const accent = spec ? nodeAccent(spec.outputs, spec.inputs) : "var(--ink-3)";
  const produces = spec && spec.outputs.some((o) => ["image", "video", "audio"].includes(o.type));
  const isAsset = spec?.contextualCreationRoute === "asset_library";
  const isGeneration = spec?.ref.id === "image.generate_from_text"
    || spec?.ref.id === "video.generate_from_image"
    || spec?.ref.id === "audio.synthesize_speech_from_text";

  return (
    <aside className="insp">
      <div className="insp__head">
        <span className="insp__badge" style={{ background: accent }} />
        <b>{spec?.label ?? node.type}</b>
      </div>

      {isAsset ? (
        <div className="insp__asset">
          <span className="insp__label">Asset</span>
          <b>
            {node.assetPresentation?.available === false
              ? "Asset unavailable"
              : node.assetPresentation?.title ?? `Untitled ${spec?.selector?.type_id.toLowerCase() ?? "asset"}`}
          </b>
          <button className="insp__asset-action" onClick={onOpenAssets}>
            Open in Assets
          </button>
        </div>
      ) : spec?.selector ? (
        <label className="insp__field">
          <span className="insp__label">Mode</span>
          <select
            className="insp__input"
            aria-label={`${spec.selector.type_id} mode`}
            value={spec.selector.mode}
            onChange={(event) => onModeChange(event.target.value)}
          >
            {(modeOptions.length > 0 ? modeOptions : [spec]).map((option) => (
              <option key={option.ref.id} value={option.selector?.mode}>
                {option.selector?.mode}
              </option>
            ))}
          </select>
        </label>
      ) : null}

      {spec && spec.status.availability !== "available" && (
        <div className="insp__note" role="status">
          {spec.status.reason ?? "Capability needs repair before it can run."}
        </div>
      )}

      {isGeneration && spec ? (
        <label className="insp__field">
          <span className="insp__label">Generation profile</span>
          <GenerationProfileSelector
            capability={spec.ref}
            value={typeof node.params.generation_profile_ref === "string"
              ? node.params.generation_profile_ref
              : ""}
            onChange={(value) => onParamChange(node.id, "generation_profile_ref", value)}
          />
        </label>
      ) : null}

      {!isAsset && spec && spec.params.length > 0 ? (
        <>
          <p className="insp__grp">Parameters</p>
          <div className="insp__fields">
            {spec.params.map((param) => (
              <label key={param.name} className="insp__field">
                <span className="insp__label">{param.label}</span>
                <ParameterInput
                  spec={param}
                  className={`insp__input${param.kind === "int" || param.kind === "float" ? " is-mono" : ""}`}
                  value={node.params[param.name] ?? param.default}
                  onChange={(value) => onParamChange(node.id, param.name, value)}
                />
              </label>
            ))}
          </div>
        </>
      ) : !isAsset ? (
        <p className="insp__note-plain">This node has no parameters.</p>
      ) : null}

      {produces && !isAsset && (
        <div className="insp__note">
          Generated media saves to the Library automatically, tagged with this project and prompt.
        </div>
      )}
    </aside>
  );
}
