import { CheckCircle2, XCircle, Info, X } from "lucide-react";
import { useStore } from "../store";

export default function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="pointer-events-auto flex items-start gap-2 rounded-md border border-slate-200 bg-white p-3 text-sm shadow-lg dark:border-slate-700 dark:bg-slate-800"
        >
          {t.kind === "success" && <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-emerald-500" />}
          {t.kind === "error" && <XCircle size={16} className="mt-0.5 shrink-0 text-red-500" />}
          {t.kind === "info" && <Info size={16} className="mt-0.5 shrink-0 text-brand-600" />}
          <span className="flex-1 break-words">{t.message}</span>
          <button onClick={() => dismiss(t.id)} className="text-slate-400 hover:text-slate-600">
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
