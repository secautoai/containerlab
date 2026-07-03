import { X, AlertTriangle, XCircle, CheckCircle2 } from "lucide-react";
import { useStore } from "../store";

// LintModal shows pre-flight topology check results (errors + warnings).
export default function LintModal() {
  const lint = useStore((s) => s.lint);
  const clear = useStore((s) => s.clearLint);

  if (!lint) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4" onClick={clear}>
      <div
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg border border-slate-200 bg-white shadow-xl dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
          <span className="flex items-center gap-2 font-semibold">
            {lint.ok ? (
              <CheckCircle2 size={18} className="text-emerald-500" />
            ) : (
              <XCircle size={18} className="text-red-500" />
            )}
            Topology check
          </span>
          <button onClick={clear} className="text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="border-b border-slate-200 px-4 py-2 text-sm dark:border-slate-800">
          {lint.issues.length === 0 ? (
            <span className="text-emerald-500">No issues found — ready to deploy.</span>
          ) : (
            <span>
              <span className={lint.errors ? "text-red-500" : "text-slate-400"}>
                {lint.errors} error{lint.errors === 1 ? "" : "s"}
              </span>
              {", "}
              <span className={lint.warnings ? "text-amber-500" : "text-slate-400"}>
                {lint.warnings} warning{lint.warnings === 1 ? "" : "s"}
              </span>
            </span>
          )}
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          <ul className="space-y-1">
            {lint.issues.map((i, idx) => (
              <li key={idx} className="flex items-start gap-2 rounded-md px-2 py-1.5 text-sm">
                {i.severity === "error" ? (
                  <XCircle size={15} className="mt-0.5 shrink-0 text-red-500" />
                ) : (
                  <AlertTriangle size={15} className="mt-0.5 shrink-0 text-amber-500" />
                )}
                <span>
                  {i.node && <span className="font-mono text-xs text-slate-400">{i.node}: </span>}
                  {i.message}
                  <span className="ml-1 text-[10px] text-slate-400">[{i.code}]</span>
                </span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
