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

export interface AssetOption {
  id: string;
  title: string;
}

/** The managed_asset parameter value shape the canonical workflow serializer expects. */
function managedAssetValue(assetId: string) {
  return { kind: "managed_asset", asset_id: assetId };
}

/** Reads the bound asset id out of a node's raw parameters. */
function boundAssetId(params: Record<string, unknown>): string {
  const value = params["asset_id"];
  if (typeof value === "string") return value;
  if (value && typeof value === "object" && "asset_id" in value) {
    return String((value as { asset_id: unknown }).asset_id);
  }
  return "";
}

export interface SelectedEdge {
  id: string;
  sourceLabel: string;
  targetLabel: string;
}

export function InspectorPanel({
  node,
  onParamChange,
  onOpenAssets = () => undefined,
  onRunThroughNode = () => undefined,
  assetOptions = [],
  readinessIssues = [],
  runDisabled = false,
  onDeleteNode,
  selectedEdge = null,
  onDeleteEdge,
}: {
  node: SelectedNode | null;
  onParamChange: (nodeId: string, name: string, value: unknown) => void;
  onOpenAssets?: () => void;
  onRunThroughNode?: (nodeId: string) => void;
  assetOptions?: AssetOption[];
  readinessIssues?: string[];
  runDisabled?: boolean;
  onDeleteNode?: (nodeId: string) => void;
  selectedEdge?: SelectedEdge | null;
  onDeleteEdge?: (edgeId: string) => void;
}) {
  if (!node && selectedEdge) {
    return (
      <aside className="insp">
        <div className="insp__head">
          <span className="insp__badge" style={{ background: "var(--ink-3)" }} />
          <b>Connection</b>
        </div>
        <p className="insp__note-plain">
          {selectedEdge.sourceLabel} → {selectedEdge.targetLabel}
        </p>
        <button
          className="insp__asset-action insp__asset-action--danger"
          onClick={() => onDeleteEdge?.(selectedEdge.id)}
        >
          Delete connection
        </button>
        <p className="insp__hint">Backspace also deletes the selection.</p>
      </aside>
    );
  }
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
          <label className="insp__field">
            <span className="insp__label">Asset</span>
            <select
              className="insp__input"
              aria-label="Asset"
              value={boundAssetId(node.params)}
              onChange={(event) => {
                if (event.target.value) {
                  onParamChange(node.id, "asset_id", managedAssetValue(event.target.value));
                }
              }}
            >
              <option value="">Choose an asset</option>
              {assetOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.title}
                </option>
              ))}
            </select>
          </label>
          {node.assetPresentation && boundAssetId(node.params) ? (
            <p className="insp__asset-meta">
              {node.assetPresentation.available
                ? node.assetPresentation.title
                : "The selected asset is not available."}
            </p>
          ) : null}
          <button className="insp__asset-action" onClick={onOpenAssets}>
            Open in Assets
          </button>
        </div>
      ) : null}

      {spec && spec.status.availability !== "available" && (
        <div className="insp__note" role="status">
          {spec.status.reason ?? "Capability needs repair before it can run."}
        </div>
      )}

      {isGeneration && spec ? (
        <label className="insp__field">
          <span className="insp__label">Generation model</span>
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
            {spec.params
              .filter((param) => !isGeneration || param.name !== "generation_profile_ref")
              .map((param) => (
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
      {readinessIssues.length > 0 && (
        <div className="insp__issues" role="status" aria-label="Ready to run issues">
          <p className="insp__grp">Before this can run</p>
          <ul className="insp__issuelist">
            {readinessIssues.map((issue) => (
              <li key={issue}>{issue}</li>
            ))}
          </ul>
        </div>
      )}
      <div className="insp__actions">
        <button
          className="insp__asset-action insp__asset-action--primary"
          onClick={() => onRunThroughNode(node.id)}
          disabled={runDisabled}
          title={runDisabled ? "Fix the issues above before running" : undefined}
        >
          Run to here
        </button>
        {onDeleteNode && (
          <button
            className="insp__asset-action insp__asset-action--danger"
            onClick={() => onDeleteNode(node.id)}
          >
            Delete node
          </button>
        )}
      </div>
      {onDeleteNode && (
        <p className="insp__hint">Backspace also deletes the selection.</p>
      )}
    </aside>
  );
}
