import { useEffect, useState } from "react";
import { X, LayoutTemplate } from "lucide-react";
import { useStore } from "../store";
import { api, type Template } from "../api";

// TemplateModal shows the quick-start catalog of built-in topologies and creates
// a new lab from the selected template.
export default function TemplateModal({ onClose }: { onClose: () => void }) {
  const createFromTemplate = useStore((s) => s.createFromTemplate);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .templates()
      .then(setTemplates)
      .catch((e) => setError((e as Error).message));
  }, []);

  const byCategory = templates.reduce<Record<string, Template[]>>((acc, t) => {
    (acc[t.category] ||= []).push(t);
    return acc;
  }, {});

  const pick = async (t: Template) => {
    await createFromTemplate(t.id);
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
            <LayoutTemplate size={18} className="text-brand" /> Start from a template
          </span>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          {error && <p className="text-sm text-red-500">{error}</p>}
          {Object.entries(byCategory).map(([cat, tpls]) => (
            <div key={cat} className="mb-4">
              <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">{cat}</div>
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                {tpls.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => pick(t)}
                    className="rounded-md border border-slate-200 bg-slate-50 p-3 text-left hover:border-brand dark:border-slate-700 dark:bg-slate-800"
                  >
                    <div className="text-sm font-medium">{t.name}</div>
                    <div className="mt-0.5 text-xs text-slate-400">{t.description}</div>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
