import { useEffect, useState } from "react";
import { X, Code2, Loader2 } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";

// YamlEditorModal lets the user view and edit the raw containerlab YAML of the
// current lab and apply it back to the canvas.
export default function YamlEditorModal() {
  const open = useStore((s) => s.yamlEditorOpen);
  const toggle = useStore((s) => s.toggleYamlEditor);
  const applyYaml = useStore((s) => s.applyYaml);
  const graph = useStore((s) => s.graph);

  const [yaml, setYaml] = useState("");
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open || !graph) return;
    setError("");
    setLoading(true);
    api
      .getYamlText(graph.name)
      .then(setYaml)
      .catch((e) => setError((e as Error).message))
      .finally(() => setLoading(false));
  }, [open, graph?.name]);

  if (!open || !graph) return null;

  const apply = async () => {
    setApplying(true);
    setError("");
    try {
      await applyYaml(yaml);
      toggle(false);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setApplying(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4" onClick={() => toggle(false)}>
      <div
        className="flex h-[85vh] w-full max-w-3xl flex-col rounded-lg border border-slate-200 bg-white shadow-xl dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
          <span className="flex items-center gap-2 font-semibold">
            <Code2 size={18} className="text-brand" /> Edit YAML — {graph.name}
          </span>
          <button onClick={() => toggle(false)} className="text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="min-h-0 flex-1 p-3">
          {loading ? (
            <div className="flex h-full items-center justify-center text-slate-400">
              <Loader2 className="animate-spin" />
            </div>
          ) : (
            <textarea
              value={yaml}
              onChange={(e) => setYaml(e.target.value)}
              spellCheck={false}
              className="h-full w-full resize-none rounded-md border border-slate-300 bg-slate-50 p-2 font-mono text-xs outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
            />
          )}
        </div>

        {error && <div className="px-4 pb-1 text-xs text-red-500">{error}</div>}

        <div className="flex justify-end gap-2 border-t border-slate-200 px-4 py-3 dark:border-slate-800">
          <button
            onClick={() => toggle(false)}
            className="rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-700"
          >
            Cancel
          </button>
          <button
            onClick={apply}
            disabled={applying || loading}
            className="flex items-center gap-1.5 rounded-md bg-brand px-3 py-1.5 text-sm font-medium text-slate-900 hover:bg-brand-600 disabled:opacity-40"
          >
            {applying && <Loader2 size={14} className="animate-spin" />} Apply
          </button>
        </div>
      </div>
    </div>
  );
}
