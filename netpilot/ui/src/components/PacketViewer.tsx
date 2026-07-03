// Packet list modal: decoded summary of a capture file.

import { useEffect, useState } from 'react'
import { Download, RefreshCw, X } from 'lucide-react'
import { api } from '../api'

interface Packet {
  ts: number
  len: number
  src: string
  dst: string
  proto: string
  info: string
}

const protoColor: Record<string, string> = {
  ICMP: 'text-emerald-400',
  TCP: 'text-cyan-400',
  UDP: 'text-violet-400',
  ARP: 'text-amber-400',
  OSPF: 'text-rose-400',
  IPv6: 'text-blue-400',
  DNS: 'text-teal-300',
}

export default function PacketViewer({
  labId,
  nodeId,
  iface,
  ifaceName,
  onClose,
}: {
  labId: string
  nodeId: string
  iface: number
  ifaceName: string
  onClose: () => void
}) {
  const [packets, setPackets] = useState<Packet[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = async () => {
    try {
      const res = await fetch(`/api/labs/${labId}/nodes/${nodeId}/interfaces/${iface}/capture/summary`)
      if (!res.ok) {
        setError((await res.json()).error ?? `HTTP ${res.status}`)
        return
      }
      setPackets(await res.json())
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  useEffect(() => {
    void refresh()
    const t = setInterval(refresh, 2000)
    return () => clearInterval(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [labId, nodeId, iface])

  const t0 = packets?.[0]?.ts ?? 0

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="flex max-h-[80vh] w-full max-w-3xl flex-col rounded-xl border border-ink-700 bg-ink-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-ink-800 px-4 py-2.5">
          <h3 className="text-sm font-medium text-white">
            Packets on <span className="font-mono text-accent-500">{ifaceName}</span>
          </h3>
          <span className="text-xs text-ink-500">{packets?.length ?? 0} shown · live</span>
          <div className="ml-auto flex gap-1">
            <button onClick={() => void refresh()} className="rounded p-1.5 text-ink-400 hover:bg-ink-700 hover:text-white" title="Refresh">
              <RefreshCw size={14} />
            </button>
            <a
              href={api.captureUrl(labId, nodeId, iface)}
              className="rounded p-1.5 text-ink-400 hover:bg-ink-700 hover:text-white"
              title="Download pcap for Wireshark"
            >
              <Download size={14} />
            </a>
            <button onClick={onClose} className="rounded p-1.5 text-ink-400 hover:bg-ink-700 hover:text-white">
              <X size={14} />
            </button>
          </div>
        </div>
        <div className="min-h-32 flex-1 overflow-y-auto p-2">
          {error && <p className="p-4 text-center text-xs text-amber-400">{error}</p>}
          {packets && packets.length === 0 && (
            <p className="p-4 text-center text-xs text-ink-500">No packets captured yet.</p>
          )}
          {packets && packets.length > 0 && (
            <table className="w-full text-left font-mono text-[11px]">
              <thead className="sticky top-0 bg-ink-900 text-[9px] uppercase tracking-wider text-ink-500">
                <tr>
                  <th className="px-2 pb-1 font-medium">time</th>
                  <th className="px-2 pb-1 font-medium">source</th>
                  <th className="px-2 pb-1 font-medium">destination</th>
                  <th className="px-2 pb-1 font-medium">proto</th>
                  <th className="px-2 pb-1 font-medium">len</th>
                  <th className="px-2 pb-1 font-medium">info</th>
                </tr>
              </thead>
              <tbody>
                {packets.map((p, i) => (
                  <tr key={i} className="border-t border-ink-850 text-ink-300 hover:bg-ink-850">
                    <td className="px-2 py-0.5 text-ink-500">{(p.ts - t0).toFixed(4)}</td>
                    <td className="px-2 py-0.5">{p.src}</td>
                    <td className="px-2 py-0.5">{p.dst}</td>
                    <td className={`px-2 py-0.5 ${protoColor[p.proto] ?? 'text-ink-400'}`}>{p.proto}</td>
                    <td className="px-2 py-0.5 text-ink-500">{p.len}</td>
                    <td className="px-2 py-0.5 text-ink-400">{p.info}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  )
}
