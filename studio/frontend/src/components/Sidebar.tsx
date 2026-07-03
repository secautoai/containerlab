import { useState } from "react";
import { Plus, Trash2, FolderOpen, CircleDot, Circle, Upload, LayoutTemplate, Copy, Pencil } from "lucide-react";
import { useStore } from "../store";
import ImportModal from "./ImportModal";
import TemplateModal from "./TemplateModal";

export default function Sidebar() {
  const labs = useStore((s) => s.labs);
  const graph = useStore((s) => s.graph);
  const openLab = useStore((s) => s.openLab);
  const createLab = useStore((s) => s.createLab);
  const deleteLab = useStore((s) => s.deleteLab);
  const cloneLab = useStore((s) => s.cloneLab);
  const renameLab = useStore((s) => s.renameLab);
  const [newName, setNewName] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [templateOpen, setTemplateOpen] = useState(false);

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    await createLab(name);
    setNewName("");
  };

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="border-b border-slate-200 p-3 dark:border-slate-800">
        <div className="flex items-center gap-2">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && create()}
            placeholder="new lab name"
            className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          />
          <button
            onClick={create}
            title="Create lab"
            className="rounded-md bg-brand px-2 py-1.5 text-slate-900 hover:bg-brand-600"
          >
            <Plus size={16} />
          </button>
        </div>
        <div className="mt-2 flex gap-2">
          <button
            onClick={() => setTemplateOpen(true)}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-md border border-slate-300 px-2 py-1.5 text-xs hover:border-brand dark:border-slate-700"
          >
            <LayoutTemplate size={14} /> Template
          </button>
          <button
            onClick={() => setImportOpen(true)}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-md border border-slate-300 px-2 py-1.5 text-xs hover:border-brand dark:border-slate-700"
          >
            <Upload size={14} /> Import
          </button>
        </div>
      </div>

      {importOpen && <ImportModal onClose={() => setImportOpen(false)} />}
      {templateOpen && <TemplateModal onClose={() => setTemplateOpen(false)} />}

      <div className="flex-1 overflow-y-auto p-2">
        <div className="px-1 pb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
          Labs ({labs.length})
        </div>
        {labs.length === 0 && (
          <p className="px-2 text-sm text-slate-400">No labs yet. Create one above.</p>
        )}
        <ul className="space-y-1">
          {labs.map((lab) => (
            <li key={lab.name}>
              <div
                className={`group flex items-center justify-between rounded-md px-2 py-1.5 text-sm hover:bg-slate-100 dark:hover:bg-slate-800 ${
                  graph?.name === lab.name ? "bg-slate-100 dark:bg-slate-800" : ""
                }`}
              >
                <button
                  className="flex min-w-0 flex-1 items-center gap-2 text-left"
                  onClick={() => openLab(lab.name)}
                >
                  {lab.deployed ? (
                    <CircleDot size={14} className="shrink-0 text-emerald-500" />
                  ) : (
                    <Circle size={14} className="shrink-0 text-slate-400" />
                  )}
                  <span className="truncate">{lab.name}</span>
                  <span className="ml-auto shrink-0 text-xs text-slate-400">{lab.nodeCount}</span>
                </button>
                <button
                  title="Open"
                  onClick={() => openLab(lab.name)}
                  className="ml-1 hidden text-slate-400 hover:text-brand group-hover:block"
                >
                  <FolderOpen size={14} />
                </button>
                <button
                  title="Clone lab"
                  onClick={() => {
                    const n = prompt(`Clone "${lab.name}" as:`, `${lab.name}-copy`);
                    if (n && n.trim()) cloneLab(lab.name, n.trim());
                  }}
                  className="ml-1 hidden text-slate-400 hover:text-brand group-hover:block"
                >
                  <Copy size={14} />
                </button>
                <button
                  title="Rename lab"
                  onClick={() => {
                    const n = prompt(`Rename "${lab.name}" to:`, lab.name);
                    if (n && n.trim() && n.trim() !== lab.name) renameLab(lab.name, n.trim());
                  }}
                  className="ml-1 hidden text-slate-400 hover:text-brand group-hover:block"
                >
                  <Pencil size={14} />
                </button>
                <button
                  title="Delete lab"
                  onClick={() => {
                    if (confirm(`Delete lab "${lab.name}"?`)) deleteLab(lab.name);
                  }}
                  className="ml-1 hidden text-slate-400 hover:text-red-500 group-hover:block"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </aside>
  );
}
