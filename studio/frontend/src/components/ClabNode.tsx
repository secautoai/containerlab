import { Handle, Position, type NodeProps } from "@xyflow/react";
import { iconFor } from "./icons";
import type { GraphNode } from "../api";

export interface ClabNodeData extends Record<string, unknown> {
  node: GraphNode;
  running: boolean;
}

// ClabNode renders a topology node with an icon, name, kind and connection
// handles on all four sides so links can be drawn from any direction.
export default function ClabNode({ data, selected }: NodeProps) {
  const d = data as ClabNodeData;
  const node = d.node;
  const Icon = iconFor(node.icon || node.kind);

  const handleStyle = { width: 8, height: 8, background: "#00c9ff", border: "none" };

  return (
    <div className={`clab-node ${selected ? "selected" : ""} ${d.running ? "running" : ""}`}>
      <Handle type="target" position={Position.Left} style={handleStyle} id="l" isConnectableStart />
      <Handle type="source" position={Position.Right} style={handleStyle} id="r" />
      <Handle type="source" position={Position.Top} style={handleStyle} id="t" />
      <Handle type="target" position={Position.Bottom} style={handleStyle} id="b" isConnectableStart />
      <div className="flex flex-col items-center gap-1">
        <Icon size={22} className="text-brand-600" />
        <span className="max-w-[120px] truncate font-semibold">{node.name}</span>
        <span className="text-[10px] text-slate-400">{node.kind}</span>
        {d.running && <span className="text-[10px] text-emerald-500">running</span>}
      </div>
    </div>
  );
}
