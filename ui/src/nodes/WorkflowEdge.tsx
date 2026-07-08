// Typed workflow edge: colored by the source port's data type, with a flowing
// dash animation while the workflow is running so you can watch data move
// between nodes. This is the visual payoff of the typed-wiring signature.

import { BaseEdge, getBezierPath, type EdgeProps } from "@xyflow/react";
import "./edgeStyles.css";

export interface WorkflowEdgeData {
  color?: string;
  running?: boolean;
  [key: string]: unknown;
}

export function WorkflowEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition } = props;
  const data = (props.data ?? {}) as WorkflowEdgeData;
  const [path] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const color = data.color ?? "var(--line-strong)";
  return (
    <BaseEdge
      id={props.id}
      path={path}
      className={`wf-edge${data.running ? " is-running" : ""}`}
      style={{ stroke: color }}
    />
  );
}
