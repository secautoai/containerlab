import { X } from "lucide-react";
import { useStore } from "../store";

// ConsolePanel is a placeholder until the interactive WebSocket console
// (xterm.js) is wired up in a later milestone.
export default function ConsolePanel() {
  const consoleNode = useStore((s) => s.consoleNode);
  const openConsole = useStore((s) => s.openConsole);

  if (!consoleNode) return null;

  return (
    <div className="flex h-56 shrink-0 flex-col border-t border-slate-200 bg-black text-slate-100 dark:border-slate-800">
      <div className="flex items-center justify-between border-b border-slate-800 px-3 py-1.5 text-xs">
        <span className="font-mono">console: {consoleNode}</span>
        <button onClick={() => openConsole(undefined)} className="text-slate-400 hover:text-white">
          <X size={14} />
        </button>
      </div>
      <div className="flex-1 overflow-auto p-2 font-mono text-xs text-slate-400">
        Interactive console coming soon…
      </div>
    </div>
  );
}
