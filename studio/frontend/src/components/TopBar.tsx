import {
  Save,
  Play,
  Trash,
  Download,
  Moon,
  Sun,
  Sparkles,
  RefreshCw,
  Circle,
  CircleDot,
  ShieldCheck,
  Loader2,
  Network,
} from "lucide-react";
import { useState } from "react";
import { useStore } from "../store";
import { api } from "../api";

export default function TopBar() {
  const graph = useStore((s) => s.graph);
  const dirty = useStore((s) => s.dirty);
  const status = useStore((s) => s.status);
  const theme = useStore((s) => s.theme);
  const caps = useStore((s) => s.capabilities);
  const saveGraph = useStore((s) => s.saveGraph);
  const deploy = useStore((s) => s.deploy);
  const destroy = useStore((s) => s.destroy);
  const validate = useStore((s) => s.validate);
  const validating = useStore((s) => s.validating);
  const configureLab = useStore((s) => s.configureLab);
  const refreshStatus = useStore((s) => s.refreshStatus);
  const [cfgOpen, setCfgOpen] = useState(false);
  const toggleTheme = useStore((s) => s.toggleTheme);
  const toggleCopilot = useStore((s) => s.toggleCopilot);

  const deployed = !!status?.deployed;

  const btn =
    "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <header className="flex h-12 shrink-0 items-center gap-2 border-b border-slate-200 bg-white px-3 dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center gap-2 font-semibold">
        <span className="text-brand">Clab</span>
        <span>Studio</span>
      </div>

      <div className="mx-2 h-5 w-px bg-slate-200 dark:bg-slate-700" />

      {graph ? (
        <div className="flex items-center gap-1.5 text-sm">
          {deployed ? (
            <CircleDot size={14} className="text-emerald-500" />
          ) : (
            <Circle size={14} className="text-slate-400" />
          )}
          <span className="font-medium">{graph.name}</span>
          {dirty && <span className="text-xs text-amber-500">● unsaved</span>}
        </div>
      ) : (
        <span className="text-sm text-slate-400">No lab open</span>
      )}

      <div className="ml-auto flex items-center gap-1.5">
        <button
          className={`${btn} border border-slate-300 dark:border-slate-700`}
          disabled={!graph}
          onClick={() => saveGraph()}
        >
          <Save size={15} /> Save
        </button>
        <a
          className={`${btn} border border-slate-300 dark:border-slate-700 ${
            graph ? "" : "pointer-events-none opacity-40"
          }`}
          href={graph ? api.yamlURL(graph.name) : "#"}
        >
          <Download size={15} /> YAML
        </a>
        <button
          className={`${btn} bg-emerald-500 text-white hover:bg-emerald-600`}
          disabled={!graph || !caps?.runtimeAvailable}
          title={caps?.runtimeAvailable ? "Deploy lab" : caps?.reason || "runtime unavailable"}
          onClick={() => deploy()}
        >
          <Play size={15} /> Deploy
        </button>
        <button
          className={`${btn} border border-red-300 text-red-500 hover:bg-red-50 dark:border-red-800 dark:hover:bg-red-950`}
          disabled={!graph || !deployed}
          onClick={() => destroy()}
        >
          <Trash size={15} /> Destroy
        </button>
        <button
          className={`${btn} border border-slate-300 dark:border-slate-700`}
          disabled={!graph || !deployed || validating}
          onClick={() => validate()}
          title="Validate end-to-end reachability"
        >
          {validating ? <Loader2 size={15} className="animate-spin" /> : <ShieldCheck size={15} />}
          Validate
        </button>
        <div className="relative">
          <button
            className={`${btn} border border-slate-300 dark:border-slate-700`}
            disabled={!graph}
            onClick={() => setCfgOpen((v) => !v)}
            title="Assign IP addressing / generate config"
          >
            <Network size={15} /> Auto-config
          </button>
          {cfgOpen && graph && (
            <div className="absolute right-0 z-20 mt-1 w-52 rounded-md border border-slate-200 bg-white p-1 text-sm shadow-lg dark:border-slate-700 dark:bg-slate-800">
              {(
                [
                  ["none", "Assign IP addressing"],
                  ["ospf", "Addressing + OSPF (FRR)"],
                  ["bgp", "Addressing + BGP (FRR)"],
                ] as const
              ).map(([proto, label]) => (
                <button
                  key={proto}
                  onClick={() => {
                    setCfgOpen(false);
                    configureLab(proto);
                  }}
                  className="block w-full rounded px-2 py-1.5 text-left hover:bg-slate-100 dark:hover:bg-slate-700"
                >
                  {label}
                </button>
              ))}
            </div>
          )}
        </div>
        <button
          className={`${btn} border border-slate-300 dark:border-slate-700`}
          disabled={!graph}
          onClick={() => refreshStatus()}
          title="Refresh status"
        >
          <RefreshCw size={15} />
        </button>

        <div className="mx-1 h-5 w-px bg-slate-200 dark:bg-slate-700" />

        <button
          className={`${btn} bg-brand text-slate-900 hover:bg-brand-600`}
          onClick={() => toggleCopilot()}
        >
          <Sparkles size={15} /> Copilot
        </button>
        <button
          className={`${btn} border border-slate-300 dark:border-slate-700`}
          onClick={() => toggleTheme()}
          title="Toggle theme"
        >
          {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
        </button>
      </div>
    </header>
  );
}
