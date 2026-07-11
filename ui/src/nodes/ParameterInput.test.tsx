import { fireEvent, render, screen } from "@testing-library/react";
import { ReactFlowProvider, type NodeProps } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import { InspectorPanel } from "../components/InspectorPanel.tsx";
import type { ParamSpec } from "./catalog.ts";
import { ParameterInput, parseParameterInput } from "./ParameterInput.tsx";
import { WorkflowFlowNode, type FlowNodeData } from "./WorkflowFlowNode.tsx";

describe("parseParameterInput", () => {
  it("keeps integer and float parameters as finite numbers", () => {
    expect(parseParameterInput("int", "36")).toEqual({ ok: true, value: 36 });
    expect(parseParameterInput("float", "2.5")).toEqual({ ok: true, value: 2.5 });
    expect(parseParameterInput("int", "2.5")).toEqual({ ok: false });
    expect(parseParameterInput("float", "Infinity")).toEqual({ ok: false });
  });
});

describe("ParameterInput", () => {
  it("emits typed numeric values for every consumer", () => {
    const onChange = vi.fn();
    const spec: ParamSpec = { name: "steps", label: "Steps", kind: "int", default: 28 };
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
        }}
        onParamChange={onParamChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Duration (s)"), { target: { value: "3.5" } });

    expect(onParamChange).toHaveBeenCalledWith("video", "duration", 3.5);
  });
});
