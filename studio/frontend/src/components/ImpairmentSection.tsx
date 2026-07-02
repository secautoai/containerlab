import { useMemo, useState } from "react";
import { Waves } from "lucide-react";
import { useStore } from "../store";

// ImpairmentSection lets the user apply netem link impairments (delay, jitter,
// loss, rate, corruption) to one of a node's interfaces on a deployed lab.
export default function ImpairmentSection({ node }: { node: string }) {
  const graph = useStore((s) => s.graph);
  const impairNode = useStore((s) => s.impairNode);

  // Interfaces this node uses, derived from its links.
  const interfaces = useMemo(() => {
    const set = new Set<string>();
    for (const l of graph?.links ?? []) {
      if (l.source === node && l.sourceEndpoint) set.add(l.sourceEndpoint);
      if (l.target === node && l.targetEndpoint) set.add(l.targetEndpoint);
    }
    return Array.from(set).sort();
  }, [graph, node]);

  const [iface, setIface] = useState("");
  const [delay, setDelay] = useState("");
  const [jitter, setJitter] = useState("");
  const [loss, setLoss] = useState("");
  const [rate, setRate] = useState("");
  const [corruption, setCorruption] = useState("");

  const selected = iface || interfaces[0] || "";

  const num = (v: string) => (v.trim() === "" ? 0 : Number(v));

  const apply = (clear = false) =>
    impairNode(node, {
      interface: selected,
      delayMs: clear ? 0 : num(delay),
      jitterMs: clear ? 0 : num(jitter),
      lossPct: clear ? 0 : num(loss),
      rateKbit: clear ? 0 : num(rate),
      corruptionPct: clear ? 0 : num(corruption),
    });

  const inputCls =
    "w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800";

  if (interfaces.length === 0) return null;

  return (
    <details className="rounded-md border border-slate-200 dark:border-slate-700">
      <summary className="flex cursor-pointer items-center gap-2 px-2 py-1.5 text-xs font-medium text-slate-500">
        <Waves size={13} /> Link impairments
      </summary>
      <div className="space-y-2 p-2">
        <label className="block">
          <span className="mb-1 block text-[11px] text-slate-400">Interface</span>
          <select value={selected} onChange={(e) => setIface(e.target.value)} className={inputCls}>
            {interfaces.map((i) => (
              <option key={i} value={i}>
                {i}
              </option>
            ))}
          </select>
        </label>
        <div className="grid grid-cols-2 gap-2">
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Delay (ms)</span>
            <input value={delay} onChange={(e) => setDelay(e.target.value)} className={inputCls} inputMode="numeric" />
          </label>
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Jitter (ms)</span>
            <input value={jitter} onChange={(e) => setJitter(e.target.value)} className={inputCls} inputMode="numeric" />
          </label>
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Loss (%)</span>
            <input value={loss} onChange={(e) => setLoss(e.target.value)} className={inputCls} inputMode="decimal" />
          </label>
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Rate (kbit)</span>
            <input value={rate} onChange={(e) => setRate(e.target.value)} className={inputCls} inputMode="numeric" />
          </label>
          <label className="col-span-2 block">
            <span className="mb-1 block text-[11px] text-slate-400">Corruption (%)</span>
            <input value={corruption} onChange={(e) => setCorruption(e.target.value)} className={inputCls} inputMode="decimal" />
          </label>
        </div>
        <div className="flex gap-1.5">
          <button
            onClick={() => apply(false)}
            className="flex-1 rounded-md bg-brand px-2 py-1.5 text-xs font-medium text-slate-900 hover:bg-brand-600"
          >
            Apply
          </button>
          <button
            onClick={() => apply(true)}
            className="flex-1 rounded-md border border-slate-300 px-2 py-1.5 text-xs hover:border-brand dark:border-slate-700"
          >
            Clear
          </button>
        </div>
      </div>
    </details>
  );
}
