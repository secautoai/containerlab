import { useMemo, useState } from "react";
import { Gauge } from "lucide-react";
import { useStore } from "../store";

// TrafficTestSection runs an iperf3 throughput test from this node to another
// running node (both need iperf3 in their image, e.g. network-multitool).
export default function TrafficTestSection({ node }: { node: string }) {
  const graph = useStore((s) => s.graph);
  const status = useStore((s) => s.status);
  const throughputTest = useStore((s) => s.throughputTest);

  // Candidate targets: other running nodes.
  const targets = useMemo(() => {
    const running = new Set(
      (status?.nodes ?? [])
        .filter((n) => n.state?.toLowerCase().includes("running"))
        .map((n) => n.name),
    );
    return (graph?.nodes ?? []).map((n) => n.name).filter((n) => n !== node && running.has(n));
  }, [graph, status, node]);

  const [target, setTarget] = useState("");
  const selected = target || targets[0] || "";

  if (targets.length === 0) return null;

  return (
    <details className="rounded-md border border-slate-200 dark:border-slate-700">
      <summary className="flex cursor-pointer items-center gap-2 px-2 py-1.5 text-xs font-medium text-slate-500">
        <Gauge size={13} /> Traffic test (iperf3)
      </summary>
      <div className="space-y-2 p-2">
        <label className="block">
          <span className="mb-1 block text-[11px] text-slate-400">Target node</span>
          <select
            value={selected}
            onChange={(e) => setTarget(e.target.value)}
            className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          >
            {targets.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </label>
        <button
          onClick={() => throughputTest(node, selected)}
          className="w-full rounded-md bg-brand px-2 py-1.5 text-xs font-medium text-slate-900 hover:bg-brand-600"
        >
          Run iperf3 {node} → {selected}
        </button>
        <p className="text-[10px] text-slate-400">
          Requires iperf3 in both node images (e.g. network-multitool).
        </p>
      </div>
    </details>
  );
}
