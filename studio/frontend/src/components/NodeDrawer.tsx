import { useEffect, useState } from "react";
import { X, Terminal, Trash2 } from "lucide-react";
import { useStore } from "../store";
import type { GraphNode } from "../api";

// NodeDrawer edits the properties of the currently selected node.
export default function NodeDrawer() {
  const graph = useStore((s) => s.graph);
  const selectedNode = useStore((s) => s.selectedNode);
  const status = useStore((s) => s.status);
  const updateNode = useStore((s) => s.updateNode);
  const removeNode = useStore((s) => s.removeNode);
  const selectNode = useStore((s) => s.selectNode);
  const openConsole = useStore((s) => s.openConsole);
  const catalog = useStore((s) => s.catalog);

  const node = graph?.nodes.find((n) => n.name === selectedNode);
  const [local, setLocal] = useState<GraphNode | undefined>(node);

  useEffect(() => setLocal(node), [node?.name, selectedNode]);

  if (!node || !local) return null;

  const rt = status?.nodes.find((n) => n.name === node.name);
  const running = !!rt?.state?.toLowerCase().includes("running");

  const commit = (patch: Partial<GraphNode>) => updateNode(node.name, patch);

  const field = (label: string, value: string, onChange: (v: string) => void, onBlur: () => void) => (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-slate-400">{label}</span>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
      />
    </label>
  );

  return (
    <aside className="flex w-72 shrink-0 flex-col border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center justify-between border-b border-slate-200 px-3 py-2 dark:border-slate-800">
        <span className="text-sm font-semibold">Node properties</span>
        <button onClick={() => selectNode(undefined)} className="text-slate-400 hover:text-slate-600">
          <X size={16} />
        </button>
      </div>

      <div className="flex-1 space-y-3 overflow-y-auto p-3">
        {field(
          "Name",
          local.name,
          (v) => setLocal({ ...local, name: v }),
          () => local.name && local.name !== node.name && commit({ name: local.name }),
        )}

        <label className="block">
          <span className="mb-1 block text-xs font-medium text-slate-400">Kind</span>
          <select
            value={local.kind}
            onChange={(e) => {
              setLocal({ ...local, kind: e.target.value });
              commit({ kind: e.target.value });
            }}
            className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          >
            {catalog.map((k) => (
              <option key={k.kind} value={k.kind}>
                {k.displayName}
              </option>
            ))}
          </select>
        </label>

        {field(
          "Image",
          local.image ?? "",
          (v) => setLocal({ ...local, image: v }),
          () => commit({ image: local.image }),
        )}
        {field(
          "Type",
          local.type ?? "",
          (v) => setLocal({ ...local, type: v }),
          () => commit({ type: local.type }),
        )}
        {field(
          "Mgmt IPv4",
          local.mgmtIpv4 ?? "",
          (v) => setLocal({ ...local, mgmtIpv4: v }),
          () => commit({ mgmtIpv4: local.mgmtIpv4 }),
        )}

        <label className="block">
          <span className="mb-1 block text-xs font-medium text-slate-400">Exec (one per line)</span>
          <textarea
            rows={3}
            value={(local.exec ?? []).join("\n")}
            onChange={(e) => setLocal({ ...local, exec: e.target.value.split("\n") })}
            onBlur={() =>
              commit({ exec: (local.exec ?? []).map((s) => s.trim()).filter(Boolean) })
            }
            className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 font-mono text-xs outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          />
        </label>

        {rt && (
          <div className="rounded-md border border-slate-200 bg-slate-50 p-2 text-xs dark:border-slate-700 dark:bg-slate-800">
            <div className="mb-1 font-medium text-slate-400">Runtime</div>
            <div>State: {rt.state}</div>
            {rt.ipv4Address && <div>IPv4: {rt.ipv4Address}</div>}
          </div>
        )}
      </div>

      <div className="space-y-2 border-t border-slate-200 p-3 dark:border-slate-800">
        <button
          disabled={!running}
          onClick={() => openConsole(node.name)}
          className="flex w-full items-center justify-center gap-2 rounded-md bg-brand px-2 py-2 text-sm font-medium text-slate-900 hover:bg-brand-600 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Terminal size={15} /> Open console
        </button>
        <button
          onClick={() => removeNode(node.name)}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-red-300 px-2 py-2 text-sm font-medium text-red-500 hover:bg-red-50 dark:border-red-800 dark:hover:bg-red-950"
        >
          <Trash2 size={15} /> Delete node
        </button>
      </div>
    </aside>
  );
}
