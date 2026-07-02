import { useCallback, useMemo, useRef } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  Controls,
  MiniMap,
  Panel,
  type Node,
  type Edge,
  type Connection,
  type NodeChange,
  useReactFlow,
} from "@xyflow/react";
import { Search } from "lucide-react";
import { useStore } from "../store";
import { matchNodes } from "../search";
import ClabNode, { type ClabNodeData } from "./ClabNode";

const nodeTypes = { clab: ClabNode };

function CanvasInner() {
  const graph = useStore((s) => s.graph);
  const status = useStore((s) => s.status);
  const selectedNode = useStore((s) => s.selectedNode);
  const selectNode = useStore((s) => s.selectNode);
  const addLink = useStore((s) => s.addLink);
  const removeLink = useStore((s) => s.removeLink);
  const removeNode = useStore((s) => s.removeNode);
  const updateNode = useStore((s) => s.updateNode);
  const addNode = useStore((s) => s.addNode);
  const catalog = useStore((s) => s.catalog);
  const searchQuery = useStore((s) => s.searchQuery);
  const setSearch = useStore((s) => s.setSearch);

  const wrapper = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition } = useReactFlow();

  const matched = useMemo(
    () => (graph ? matchNodes(graph.nodes, searchQuery) : new Set<string>()),
    [graph, searchQuery],
  );
  const searching = searchQuery.trim() !== "";

  const runningSet = useMemo(() => {
    const set = new Set<string>();
    status?.nodes?.forEach((n) => {
      if (n.state && n.state.toLowerCase().includes("running")) set.add(n.name);
    });
    return set;
  }, [status]);

  const nodes: Node<ClabNodeData>[] = useMemo(() => {
    if (!graph) return [];
    return graph.nodes.map((n) => ({
      id: n.name,
      type: "clab",
      position: n.position || { x: 0, y: 0 },
      selected: n.name === selectedNode,
      data: {
        node: n,
        running: runningSet.has(n.name),
        dimmed: searching && !matched.has(n.name),
        matched: searching && matched.has(n.name),
      },
    }));
  }, [graph, selectedNode, runningSet, searching, matched]);

  const edges: Edge[] = useMemo(() => {
    if (!graph) return [];
    return graph.links.map((l, i) => ({
      id: `e${i}`,
      source: l.source,
      target: l.target,
      label: `${l.sourceEndpoint ?? ""} — ${l.targetEndpoint ?? ""}`,
      labelStyle: { fontSize: 9, fill: "#94a3b8" },
      style: { stroke: "#64748b", strokeWidth: 1.5 },
      data: { index: i },
    }));
  }, [graph]);

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      for (const c of changes) {
        if (c.type === "position" && c.position && c.dragging === false) {
          updateNode(c.id, { position: c.position });
        }
        if (c.type === "select" && c.selected) {
          selectNode(c.id);
        }
        if (c.type === "remove") {
          removeNode(c.id);
        }
      }
    },
    [updateNode, selectNode, removeNode],
  );

  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target || c.source === c.target) return;
      addLink({ source: c.source, target: c.target });
    },
    [addLink],
  );

  const onEdgesDelete = useCallback(
    (deleted: Edge[]) => {
      // remove by descending index to keep indices valid
      const indices = deleted
        .map((e) => (e.data as { index: number } | undefined)?.index)
        .filter((i): i is number => typeof i === "number")
        .sort((a, b) => b - a);
      indices.forEach((i) => removeLink(i));
    },
    [removeLink],
  );

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const kindId = e.dataTransfer.getData("application/clab-kind");
      const kind = catalog.find((k) => k.kind === kindId);
      if (!kind) return;
      const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });
      addNode(kind, position);
    },
    [catalog, addNode, screenToFlowPosition],
  );

  if (!graph) {
    return (
      <div className="flex flex-1 items-center justify-center text-slate-400">
        <div className="text-center">
          <p className="text-lg font-medium">No lab open</p>
          <p className="text-sm">Create or open a lab, or ask the Copilot to build one.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="relative flex-1" ref={wrapper} onDrop={onDrop} onDragOver={(e) => e.preventDefault()}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onConnect={onConnect}
        onEdgesDelete={onEdgesDelete}
        onPaneClick={() => selectNode(undefined)}
        fitView
        proOptions={{ hideAttribution: true }}
        deleteKeyCode={["Backspace", "Delete"]}
      >
        <Panel position="top-left">
          <div className="flex items-center gap-2 rounded-md border border-slate-300 bg-white/90 px-2 py-1 shadow-sm backdrop-blur dark:border-slate-700 dark:bg-slate-800/90">
            <Search size={14} className="text-slate-400" />
            <input
              value={searchQuery}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="search nodes…"
              className="w-40 bg-transparent text-sm outline-none"
            />
            {searching && (
              <span className="text-xs text-slate-400">{matched.size} match{matched.size === 1 ? "" : "es"}</span>
            )}
          </div>
        </Panel>
        <Background gap={18} color="#334155" />
        <Controls className="!bg-white dark:!bg-slate-800" />
        <MiniMap
          pannable
          zoomable
          className="!bg-slate-100 dark:!bg-slate-800"
          nodeColor={() => "#00c9ff"}
        />
      </ReactFlow>
    </div>
  );
}

export default function Canvas() {
  return (
    <ReactFlowProvider>
      <CanvasInner />
    </ReactFlowProvider>
  );
}
