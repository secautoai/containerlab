import { X, Check, XCircle, ShieldCheck } from "lucide-react";
import { useStore } from "../store";

// ValidationModal shows the end-to-end reachability report for a lab.
export default function ValidationModal() {
  const report = useStore((s) => s.validation);
  const clear = useStore((s) => s.clearValidation);

  if (!report) return null;

  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4"
      onClick={clear}
    >
      <div
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg border border-slate-200 bg-white shadow-xl dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
          <span className="flex items-center gap-2 font-semibold">
            <ShieldCheck size={18} className="text-brand" /> Validation — {report.lab}
          </span>
          <button onClick={clear} className="text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="border-b border-slate-200 px-4 py-2 text-sm dark:border-slate-800">
          <span
            className={report.failed === 0 ? "text-emerald-500" : "text-red-500"}
          >
            {report.summary}
          </span>
          <span className="ml-2 text-slate-400">
            ({report.passed} passed, {report.failed} failed)
          </span>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          {report.checks.length === 0 && (
            <p className="p-3 text-sm text-slate-400">
              No checks were run. Deploy the lab and ensure nodes have management IPs.
            </p>
          )}
          <ul className="space-y-1">
            {report.checks.map((c, i) => (
              <li
                key={i}
                className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
              >
                {c.ok ? (
                  <Check size={15} className="shrink-0 text-emerald-500" />
                ) : (
                  <XCircle size={15} className="shrink-0 text-red-500" />
                )}
                <span className="font-mono text-xs">
                  {c.from} → {c.to} ({c.target})
                </span>
                {!c.ok && c.detail && (
                  <span className="ml-auto truncate text-xs text-slate-400">{c.detail}</span>
                )}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
