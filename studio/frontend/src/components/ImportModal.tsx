import { useState } from "react";
import { X, Upload } from "lucide-react";
import { useStore } from "../store";

// ImportModal lets the user paste or upload a containerlab topology YAML to
// create a new lab that renders on the canvas (with auto-layout).
export default function ImportModal({ onClose }: { onClose: () => void }) {
  const importLab = useStore((s) => s.importLab);
  const [yaml, setYaml] = useState("");
  const [name, setName] = useState("");

  const onFile = (f?: File) => {
    if (!f) return;
    const reader = new FileReader();
    reader.onload = () => setYaml(String(reader.result ?? ""));
    reader.readAsText(f);
  };

  const doImport = async () => {
    if (!yaml.trim()) return;
    await importLab(yaml, name.trim() || undefined);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4" onClick={onClose}>
      <div
        className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-lg border border-slate-200 bg-white shadow-xl dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
          <span className="flex items-center gap-2 font-semibold">
            <Upload size={18} className="text-brand" /> Import topology
          </span>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="flex-1 space-y-3 overflow-y-auto p-4">
          <div className="flex items-center gap-2">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="lab name (optional — taken from YAML if omitted)"
              className="flex-1 rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
            />
            <label className="cursor-pointer rounded-md border border-slate-300 px-2 py-1.5 text-sm hover:border-brand dark:border-slate-700">
              Upload file
              <input
                type="file"
                accept=".yml,.yaml"
                className="hidden"
                onChange={(e) => onFile(e.target.files?.[0])}
              />
            </label>
          </div>
          <textarea
            value={yaml}
            onChange={(e) => setYaml(e.target.value)}
            rows={16}
            placeholder="Paste a *.clab.yml topology here…"
            className="w-full resize-none rounded-md border border-slate-300 bg-slate-50 p-2 font-mono text-xs outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          />
        </div>

        <div className="flex justify-end gap-2 border-t border-slate-200 px-4 py-3 dark:border-slate-800">
          <button
            onClick={onClose}
            className="rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-700"
          >
            Cancel
          </button>
          <button
            onClick={doImport}
            disabled={!yaml.trim()}
            className="rounded-md bg-brand px-3 py-1.5 text-sm font-medium text-slate-900 hover:bg-brand-600 disabled:opacity-40"
          >
            Import
          </button>
        </div>
      </div>
    </div>
  );
}
