import { X, Sparkles } from "lucide-react";
import { useStore } from "../store";

// CopilotPanel is a placeholder until the AI agent endpoint is wired up in a
// later milestone.
export default function CopilotPanel() {
  const open = useStore((s) => s.copilotOpen);
  const toggle = useStore((s) => s.toggleCopilot);

  if (!open) return null;

  return (
    <aside className="flex w-96 shrink-0 flex-col border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center justify-between border-b border-slate-200 px-3 py-2 dark:border-slate-800">
        <span className="flex items-center gap-2 text-sm font-semibold">
          <Sparkles size={15} className="text-brand" /> Copilot
        </span>
        <button onClick={() => toggle(false)} className="text-slate-400 hover:text-slate-600">
          <X size={16} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-4 text-sm text-slate-400">
        Describe the network you want and the Copilot will build it. (Coming soon.)
      </div>
    </aside>
  );
}
