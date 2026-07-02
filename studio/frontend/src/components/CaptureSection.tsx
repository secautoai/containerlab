import { useMemo, useState } from "react";
import { ScanLine } from "lucide-react";
import { useStore } from "../store";

// CaptureSection captures packets on one of the node's interfaces and downloads
// a .pcap (requires tcpdump in the node image).
export default function CaptureSection({ node }: { node: string }) {
  const graph = useStore((s) => s.graph);
  const capturePackets = useStore((s) => s.capturePackets);

  const interfaces = useMemo(() => {
    const set = new Set<string>();
    for (const l of graph?.links ?? []) {
      if (l.source === node && l.sourceEndpoint) set.add(l.sourceEndpoint);
      if (l.target === node && l.targetEndpoint) set.add(l.targetEndpoint);
    }
    return Array.from(set).sort();
  }, [graph, node]);

  const [iface, setIface] = useState("");
  const [count, setCount] = useState("20");
  const selected = iface || interfaces[0] || "";

  if (interfaces.length === 0) return null;

  return (
    <details className="rounded-md border border-slate-200 dark:border-slate-700">
      <summary className="flex cursor-pointer items-center gap-2 px-2 py-1.5 text-xs font-medium text-slate-500">
        <ScanLine size={13} /> Packet capture
      </summary>
      <div className="space-y-2 p-2">
        <div className="grid grid-cols-2 gap-2">
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Interface</span>
            <select
              value={selected}
              onChange={(e) => setIface(e.target.value)}
              className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
            >
              {interfaces.map((i) => (
                <option key={i} value={i}>
                  {i}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-[11px] text-slate-400">Packets</span>
            <input
              value={count}
              onChange={(e) => setCount(e.target.value)}
              inputMode="numeric"
              className="w-full rounded-md border border-slate-300 bg-slate-50 px-2 py-1 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
            />
          </label>
        </div>
        <button
          onClick={() => capturePackets(node, selected, Number(count) || 20)}
          className="w-full rounded-md bg-brand px-2 py-1.5 text-xs font-medium text-slate-900 hover:bg-brand-600"
        >
          Capture &amp; download .pcap
        </button>
        <p className="text-[10px] text-slate-400">Requires tcpdump in the node image.</p>
      </div>
    </details>
  );
}
