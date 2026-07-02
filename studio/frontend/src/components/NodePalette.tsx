import { useStore } from "../store";
import { iconFor } from "./icons";
import type { KindInfo } from "../api";

// NodePalette lists available node kinds grouped by vendor. Kinds can be
// dragged onto the canvas or clicked to add at a default position.
export default function NodePalette() {
  const catalog = useStore((s) => s.catalog);
  const addNode = useStore((s) => s.addNode);

  const byVendor = catalog.reduce<Record<string, KindInfo[]>>((acc, k) => {
    (acc[k.vendor] ||= []).push(k);
    return acc;
  }, {});

  return (
    <div className="w-52 shrink-0 overflow-y-auto border-r border-slate-200 bg-white p-2 dark:border-slate-800 dark:bg-slate-900">
      <div className="px-1 pb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
        Node palette
      </div>
      {Object.entries(byVendor).map(([vendor, kinds]) => (
        <div key={vendor} className="mb-3">
          <div className="px-1 pb-1 text-[11px] font-medium text-slate-400">{vendor}</div>
          <div className="space-y-1">
            {kinds.map((k) => {
              const Icon = iconFor(k.icon);
              return (
                <button
                  key={k.kind}
                  title={k.description}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData("application/clab-kind", k.kind);
                    e.dataTransfer.effectAllowed = "move";
                  }}
                  onClick={() => addNode(k, { x: 120 + Math.random() * 240, y: 80 + Math.random() * 200 })}
                  className="flex w-full items-center gap-2 rounded-md border border-slate-200 bg-slate-50 px-2 py-1.5 text-left text-xs hover:border-brand hover:bg-brand-50 dark:border-slate-700 dark:bg-slate-800 dark:hover:bg-slate-700"
                >
                  <Icon size={16} className="shrink-0 text-brand-600" />
                  <span className="truncate">{k.displayName}</span>
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
