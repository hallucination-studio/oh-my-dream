import { fireEvent, render, screen } from "@testing-library/react";
import { ReactFlowProvider, type NodeProps } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { InspectorPanel } from "../components/InspectorPanel.tsx";
import { nodeSpecFromBundle, type ParamSpec } from "./catalog.ts";
import { ParameterInput, parseParameterInput } from "./ParameterInput.tsx";
import { WorkflowFlowNode, type FlowNodeData } from "./WorkflowFlowNode.tsx";

describe("parseParameterInput", () => {
  it("keeps integer and float parameters as finite numbers", () => {
    expect(parseParameterInput(parameter("int"), "36")).toEqual({ ok: true, value: 36 });
    expect(parseParameterInput(parameter("float"), "2.5")).toEqual({ ok: true, value: 2.5 });
    expect(parseParameterInput(parameter("int"), "2.5")).toMatchObject({ ok: false });
    expect(parseParameterInput(parameter("float"), "Infinity")).toMatchObject({ ok: false });
  });

  it("parses generated enum, nullable, text, and constrained values", () => {
    const enumSpec: ParamSpec = {
      name: "quality",
      label: "Quality",
      kind: "enum",
      options: ["draft", "final"],
      nullable: false,
      required: true,
      constraints: {},
    };
    const nullableSpec: ParamSpec = {
      name: "caption",
      label: "Caption",
      kind: "text",
      nullable: true,
      required: false,
      constraints: {},
    };
    const boundedSpec: ParamSpec = {
      name: "steps",
      label: "Steps",
      kind: "int",
      nullable: false,
      required: true,
      constraints: { minimum: 1, maximum: 64 },
    };

    expect(parseParameterInput(enumSpec, "final")).toEqual({ ok: true, value: "final" });
    expect(parseParameterInput(enumSpec, "other")).toMatchObject({ ok: false });
    expect(parseParameterInput(nullableSpec, "")).toEqual({ ok: true, value: null });
    expect(parseParameterInput(nullableSpec, "caption")).toEqual({ ok: true, value: "caption" });
    expect(parseParameterInput(boundedSpec, "0")).toMatchObject({ ok: false });
    expect(parseParameterInput(boundedSpec, "32")).toEqual({ ok: true, value: 32 });
  });
});

describe("ParameterInput", () => {
  it("emits typed numeric values for every consumer", () => {
    const onChange = vi.fn();
    const spec: ParamSpec = {
      name: "steps",
      label: "Steps",
      kind: "int",
      default: 28,
      nullable: false,
      required: true,
      constraints: {},
    };
    render(<ParameterInput spec={spec} value={28} onChange={onChange} />);

    fireEvent.change(screen.getByLabelText("Steps"), { target: { value: "40" } });

    expect(onChange).toHaveBeenCalledWith(40);
  });

  it("keeps node-body integer edits typed", () => {
    const onParamChange = vi.fn();
    const props = {
      id: "image",
      selected: false,
      data: {
        type: "TextToImage",
        capability: nodeSpec("TextToImage"),
        params: { model: "mock-image", steps: 28, seed: 42 },
        onParamChange,
      } satisfies FlowNodeData,
    } as unknown as NodeProps;
    render(
      <ReactFlowProvider>
        <WorkflowFlowNode {...props} />
      </ReactFlowProvider>,
    );

    fireEvent.change(screen.getByLabelText("Steps"), { target: { value: "34" } });

    expect(onParamChange).toHaveBeenCalledWith("steps", 34);
  });

  it("keeps inspector float edits typed", () => {
    const onParamChange = vi.fn();
    render(
      <InspectorPanel
        node={{
          id: "video",
          type: "ImageToVideo",
          params: { model: "mock-video", duration: 4, fps: 24 },
          capability: nodeSpec("ImageToVideo"),
        }}
        onParamChange={onParamChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Duration"), { target: { value: "3.5" } });

    expect(onParamChange).toHaveBeenCalledWith("video", "duration", 3.5);
  });

  it("edits long-form text in a multiline control that round-trips", () => {
    const onChange = vi.fn();
    render(
      <ParameterInput
        spec={{
          name: "text",
          label: "text",
          kind: "text",
          required: true,
          constraints: { maximum: 65536 },
        }}
        className="insp__input"
        value="a sunset city"
        onChange={onChange}
      />,
    );

    const editor = screen.getByRole("textbox", { name: "text" });
    expect(editor.tagName).toBe("TEXTAREA");
    fireEvent.change(editor, { target: { value: "line one\nline two" } });
    expect(onChange).toHaveBeenLastCalledWith("line one\nline two");
  });
});

function parameter(kind: ParamSpec["kind"]): ParamSpec {
  return {
    name: "value",
    label: "Value",
    kind,
    nullable: false,
    required: true,
    constraints: {},
  };
}

function nodeSpec(type: string) {
  const catalog = catalogFixture as unknown as CapabilityCatalog;
  const entry = catalog.capabilities.find((candidate) => candidate.contract.reference.id === type);
  if (!entry) throw new Error(`missing fixture capability ${type}`);
  return nodeSpecFromBundle({
    selector: entry.selector,
    reference: entry.contract.reference,
    contract: entry.contract,
    presentation: entry.presentation,
    status: entry.status,
  });
}
