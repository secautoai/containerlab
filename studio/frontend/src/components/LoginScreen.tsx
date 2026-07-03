import { useState } from "react";
import { Lock, Loader2 } from "lucide-react";
import { useStore } from "../store";

// LoginScreen gates the app when the server requires a shared-secret token.
export default function LoginScreen() {
  const login = useStore((s) => s.login);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!token.trim() || busy) return;
    setBusy(true);
    await login(token.trim());
    setBusy(false);
  };

  return (
    <div className="flex h-full items-center justify-center bg-slate-100 dark:bg-slate-950">
      <div className="w-80 rounded-lg border border-slate-200 bg-white p-6 shadow-xl dark:border-slate-700 dark:bg-slate-900">
        <div className="mb-4 flex items-center gap-2 text-lg font-semibold">
          <span className="text-brand">Clab</span>
          <span>Studio</span>
        </div>
        <div className="mb-4 flex items-center gap-2 text-sm text-slate-400">
          <Lock size={14} /> This instance requires a token to continue.
        </div>
        <input
          type="password"
          value={token}
          onChange={(e) => setToken(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          placeholder="access token"
          autoFocus
          className="mb-3 w-full rounded-md border border-slate-300 bg-slate-50 px-3 py-2 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
        />
        <button
          onClick={submit}
          disabled={busy || !token.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md bg-brand px-3 py-2 text-sm font-medium text-slate-900 hover:bg-brand-600 disabled:opacity-40"
        >
          {busy && <Loader2 size={14} className="animate-spin" />} Sign in
        </button>
      </div>
    </div>
  );
}
