// Right-hand properties panel for the current selection
// (node / network / link / annotation).

import { useEffect, useState } from 'react'
import { Download, Play, ScrollText, Square, Terminal, X } from 'lucide-react'
import {
  api,
  ifaceName,
  type Impairment,
  type LabView,
} from '../api'
import { useStore } from '../store'
import type { Selection } from './Canvas'
import { stateColor } from './icons'

export default function SidePanel({
  selection,
  onClose,
}: {
  selection: Selection
  onClose: () => void
}) {
  const lab = useStore((s) => s.lab)
  if (!lab) return null
  return (
    <aside className="flex w-80 shrink-0 flex-col overflow-y-auto border-l border-ink-800 bg-ink-900">
      <div className="flex items-center justify-between border-b border-ink-800 px-3 py-2">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-400">
          {selection.kind} properties
        </h3>
        <button onClick={onClose} className="rounded p-1 text-ink-400 hover:bg-ink-700 hover:text-white">
          <X size={14} />
        </button>
      </div>
      {selection.kind === 'node' && <NodeProps lab={lab} nodeId={selection.id} />}
      {selection.kind === 'link' && <LinkProps lab={lab} linkId={selection.id} />}
      {selection.kind === 'network' && <NetworkProps lab={lab} netId={selection.id} />}
      {selection.kind === 'annotation' && <AnnotationProps lab={lab} annId={selection.id} />}
    </aside>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-[10px] font-medium uppercase tracking-wide text-ink-400">{label}</span>
      <div className="mt-0.5">{children}</div>
    </label>
  )
}

const inputCls =
  'w-full rounded-md border border-ink-700 bg-ink-950 px-2 py-1 text-sm text-white outline-none focus:border-accent-600'

function NodeProps({ lab, nodeId }: { lab: LabView; nodeId: string }) {
  const node = lab.nodes[nodeId]
  const states = useStore((s) => s.states)
  const templates = useStore((s) => s.templates)
  const refreshLab = useStore((s) => s.refreshLab)
  const openConsole = useStore((s) => s.openConsole)
  const pushLog = useStore((s) => s.pushLog)
  const [form, setForm] = useState({ name: '', cpus: 1, ram_mb: 1024, interfaces: 4, image: '' })
  const [config, setConfig] = useState<string | null>(null)
  const [showConfig, setShowConfig] = useState(false)

  useEffect(() => {
    if (node)
      setForm({
        name: node.name,
        cpus: node.cpus,
        ram_mb: node.ram_mb,
        interfaces: node.interfaces,
        image: node.image,
      })
  }, [node])

  if (!node) return null
  const state = states[nodeId] ?? 'stopped'
  const running = state === 'running' || state === 'starting'
  const template = templates.find((t) => t.id === node.template)

  const save = async () => {
    try {
      await api.updateNode(lab.id, nodeId, form)
      await refreshLab()
    } catch (e) {
      pushLog('error', `save: ${e instanceof Error ? e.message : e}`)
    }
  }

  return (
    <div className="space-y-3 p-3">
      <div className="flex items-center gap-2">
        <span className="h-2.5 w-2.5 rounded-full" style={{ background: stateColor[state] }} />
        <span className="font-mono text-sm text-white">{node.name}</span>
        <span className="text-xs text-ink-500">{state}</span>
        <div className="ml-auto flex gap-1">
          {!running ? (
            <button
              title="Start"
              onClick={async () => {
                try {
                  await api.startNode(lab.id, nodeId)
                } catch (e) {
                  pushLog('error', `start: ${e instanceof Error ? e.message : e}`)
                }
              }}
              className="rounded-md bg-emerald-700/70 p-1.5 text-white hover:bg-emerald-600"
            >
              <Play size={14} />
            </button>
          ) : (
            <>
              <button
                title="Console"
                onClick={() => openConsole(nodeId, node.name)}
                className="rounded-md bg-ink-700 p-1.5 text-white hover:bg-ink-600"
              >
                <Terminal size={14} />
              </button>
              <button
                title="Stop"
                onClick={() => void api.stopNode(lab.id, nodeId)}
                className="rounded-md bg-red-900/70 p-1.5 text-white hover:bg-red-800"
              >
                <Square size={14} />
              </button>
            </>
          )}
        </div>
      </div>

      <Field label="Name">
        <input className={inputCls} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
      </Field>
      <div className="grid grid-cols-2 gap-2">
        <Field label="vCPUs">
          <input
            type="number"
            min={1}
            className={inputCls}
            value={form.cpus}
            disabled={running}
            onChange={(e) => setForm({ ...form, cpus: +e.target.value })}
          />
        </Field>
        <Field label="RAM (MB)">
          <input
            type="number"
            min={64}
            step={256}
            className={inputCls}
            value={form.ram_mb}
            disabled={running}
            onChange={(e) => setForm({ ...form, ram_mb: +e.target.value })}
          />
        </Field>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Field label="Interfaces">
          <input
            type="number"
            min={1}
            max={template?.max_interfaces ?? 32}
            className={inputCls}
            value={form.interfaces}
            disabled={running}
            onChange={(e) => setForm({ ...form, interfaces: +e.target.value })}
          />
        </Field>
        <Field label="Image version">
          <select
            className={inputCls}
            value={form.image}
            disabled={running}
            onChange={(e) => setForm({ ...form, image: e.target.value })}
          >
            <option value="">(none)</option>
            {(template?.available_images ?? []).map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
            {form.image && !(template?.available_images ?? []).includes(form.image) && (
              <option value={form.image}>{form.image} (missing)</option>
            )}
          </select>
        </Field>
      </div>
      <button
        onClick={() => void save()}
        className="w-full rounded-md bg-accent-600 py-1.5 text-sm font-medium text-white hover:bg-accent-500"
      >
        Save changes
      </button>

      <button
        onClick={async () => {
          if (config === null) {
            const c = await api.nodeConfig(lab.id, nodeId)
            setConfig(c.config)
          }
          setShowConfig(!showConfig)
        }}
        className="flex w-full items-center gap-2 rounded-md border border-ink-700 py-1.5 pl-2 text-sm text-ink-300 hover:border-ink-600 hover:text-white"
      >
        <ScrollText size={14} /> Startup config {showConfig ? '▾' : '▸'}
      </button>
      {showConfig && (
        <div>
          <textarea
            className={`${inputCls} h-48 font-mono text-xs`}
            value={config ?? ''}
            onChange={(e) => setConfig(e.target.value)}
            placeholder={
              template?.qemu && template.notes ? `# ${template.notes}` : '! startup configuration'
            }
          />
          <button
            onClick={async () => {
              await api.setNodeConfig(lab.id, nodeId, config ?? '')
              pushLog('info', `${node.name}: startup config saved (applies after wipe+start)`)
            }}
            className="mt-1 w-full rounded-md bg-ink-700 py-1 text-xs text-white hover:bg-ink-600"
          >
            Save config
          </button>
        </div>
      )}

      <Interfaces lab={lab} nodeId={nodeId} running={running} />

      {template && (
        <p className="rounded-md border border-ink-800 bg-ink-950 p-2 text-[11px] leading-relaxed text-ink-500">
          {template.notes}
        </p>
      )}
    </div>
  )
}

function Interfaces({ lab, nodeId, running }: { lab: LabView; nodeId: string; running: boolean }) {
  const node = lab.nodes[nodeId]
  const templates = useStore((s) => s.templates)
  const pushLog = useStore((s) => s.pushLog)
  const [capturing, setCapturing] = useState<Record<number, boolean>>({})
  if (!node) return null
  const pattern = templates.find((t) => t.id === node.template)?.iface_pattern ?? 'eth{i}'

  const usedBy = (iface: number) => {
    for (const l of Object.values(lab.links)) {
      for (const [ep, other] of [
        [l.a, l.b],
        [l.b, l.a],
      ] as const) {
        if (ep.kind === 'node' && ep.node === nodeId && ep.iface === iface) {
          if (other.kind === 'node') {
            const peer = lab.nodes[other.node]
            return `→ ${peer?.name ?? '?'}`
          }
          return `→ ${lab.networks[other.network]?.name ?? 'net'}`
        }
      }
    }
    return null
  }

  return (
    <div>
      <span className="text-[10px] font-medium uppercase tracking-wide text-ink-400">Interfaces</span>
      <div className="mt-1 max-h-44 space-y-0.5 overflow-y-auto">
        {Array.from({ length: node.interfaces }, (_, i) => {
          const peer = usedBy(i)
          return (
            <div
              key={i}
              className="flex items-center gap-2 rounded px-1.5 py-0.5 text-xs hover:bg-ink-850"
            >
              <span className="font-mono text-ink-300">{ifaceName(pattern, i)}</span>
              <span className="truncate text-ink-600">{peer ?? 'unconnected'}</span>
              {running && peer && (
                <span className="ml-auto flex shrink-0 gap-1">
                  <button
                    title={capturing[i] ? 'Stop capture' : 'Start packet capture'}
                    className={`rounded px-1 text-[10px] ${
                      capturing[i]
                        ? 'bg-red-900 text-red-200'
                        : 'bg-ink-700 text-ink-300 hover:bg-ink-600'
                    }`}
                    onClick={async () => {
                      try {
                        if (capturing[i]) {
                          await api.stopCapture(lab.id, nodeId, i)
                          setCapturing((c) => ({ ...c, [i]: false }))
                          pushLog('info', 'capture stopped — download the pcap')
                        } else {
                          await api.startCapture(lab.id, nodeId, i)
                          setCapturing((c) => ({ ...c, [i]: true }))
                          pushLog('info', `capturing on ${ifaceName(pattern, i)}`)
                        }
                      } catch (e) {
                        pushLog('error', `capture: ${e instanceof Error ? e.message : e}`)
                      }
                    }}
                  >
                    {capturing[i] ? '⏺ rec' : 'pcap'}
                  </button>
                  <a
                    title="Download pcap"
                    href={api.captureUrl(lab.id, nodeId, i)}
                    className="rounded bg-ink-700 px-1 text-ink-300 hover:bg-ink-600"
                  >
                    <Download size={10} className="mt-0.5" />
                  </a>
                </span>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

const noImpairment: Impairment = { delay_ms: 0, jitter_ms: 0, loss_pct: 0, rate_kbit: 0 }

function LinkProps({ lab, linkId }: { lab: LabView; linkId: string }) {
  const link = lab.links[linkId]
  const refreshLab = useStore((s) => s.refreshLab)
  const [imp, setImp] = useState<Impairment>(noImpairment)

  useEffect(() => {
    setImp(link?.impairment ?? noImpairment)
  }, [link])

  if (!link) return null

  const endName = (ep: typeof link.a) =>
    ep.kind === 'node'
      ? `${lab.nodes[ep.node]?.name ?? '?'} [${ep.iface}]`
      : lab.networks[ep.network]?.name ?? 'network'

  const apply = async (next: Impairment) => {
    setImp(next)
    const isNoop =
      next.delay_ms === 0 && next.jitter_ms === 0 && next.loss_pct === 0 && next.rate_kbit === 0
    await api.updateLink(lab.id, linkId, { impairment: isNoop ? null : next })
    await refreshLab()
  }

  return (
    <div className="space-y-3 p-3">
      <div className="rounded-md border border-ink-800 bg-ink-950 p-2 text-center font-mono text-xs text-ink-300">
        {endName(link.a)} ⟷ {endName(link.b)}
      </div>
      <h4 className="text-[10px] font-semibold uppercase tracking-wider text-ink-400">
        Link quality (live)
      </h4>
      <Slider
        label={`Delay ${imp.delay_ms} ms`}
        min={0}
        max={2000}
        value={imp.delay_ms}
        onChange={(v) => void apply({ ...imp, delay_ms: v })}
      />
      <Slider
        label={`Jitter ${imp.jitter_ms} ms`}
        min={0}
        max={500}
        value={imp.jitter_ms}
        onChange={(v) => void apply({ ...imp, jitter_ms: v })}
      />
      <Slider
        label={`Loss ${imp.loss_pct}%`}
        min={0}
        max={100}
        value={imp.loss_pct}
        onChange={(v) => void apply({ ...imp, loss_pct: v })}
      />
      <Field label="Rate limit (kbit/s, 0 = unlimited)">
        <input
          type="number"
          min={0}
          step={64}
          className={inputCls}
          value={imp.rate_kbit}
          onChange={(e) => void apply({ ...imp, rate_kbit: +e.target.value })}
        />
      </Field>
      <button
        onClick={() => void apply(noImpairment)}
        className="w-full rounded-md border border-ink-700 py-1 text-xs text-ink-300 hover:text-white"
      >
        Clear impairment
      </button>
      <button
        onClick={async () => {
          await api.deleteLink(lab.id, linkId)
          await refreshLab()
        }}
        className="w-full rounded-md bg-red-900/60 py-1.5 text-sm text-red-200 hover:bg-red-800"
      >
        Delete link
      </button>
    </div>
  )
}

function Slider({
  label,
  min,
  max,
  value,
  onChange,
}: {
  label: string
  min: number
  max: number
  value: number
  onChange: (v: number) => void
}) {
  return (
    <label className="block">
      <span className="text-xs text-ink-300">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(+e.target.value)}
        className="w-full accent-cyan-500"
      />
    </label>
  )
}

function NetworkProps({ lab, netId }: { lab: LabView; netId: string }) {
  const net = lab.networks[netId]
  const refreshLab = useStore((s) => s.refreshLab)
  const [name, setName] = useState('')
  useEffect(() => {
    if (net) setName(net.name)
  }, [net])
  if (!net) return null
  return (
    <div className="space-y-3 p-3">
      <Field label="Name">
        <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Kind">
        <select
          className={inputCls}
          value={net.kind}
          onChange={async (e) => {
            await api.updateNetwork(lab.id, netId, { kind: e.target.value as never })
            await refreshLab()
          }}
        >
          <option value="bridge">bridge (L2 hub)</option>
          <option value="nat">NAT to host</option>
          <option value="management">management (NAT + DHCP)</option>
          <option value="cloud">cloud (host interface)</option>
        </select>
      </Field>
      <p className="text-[11px] leading-relaxed text-ink-500">
        NAT / management / cloud kinds reach outside the lab only when the server runs in
        privileged mode; in rootless mode they behave as isolated segments.
      </p>
      <button
        onClick={async () => {
          await api.updateNetwork(lab.id, netId, { name })
          await refreshLab()
        }}
        className="w-full rounded-md bg-accent-600 py-1.5 text-sm font-medium text-white hover:bg-accent-500"
      >
        Save
      </button>
      <button
        onClick={async () => {
          await api.deleteNetwork(lab.id, netId)
          await refreshLab()
        }}
        className="w-full rounded-md bg-red-900/60 py-1.5 text-sm text-red-200 hover:bg-red-800"
      >
        Delete network
      </button>
    </div>
  )
}

function AnnotationProps({ lab, annId }: { lab: LabView; annId: string }) {
  const ann = lab.annotations[annId]
  const refreshLab = useStore((s) => s.refreshLab)
  const [text, setText] = useState('')
  const [color, setColor] = useState('#94a3b8')
  useEffect(() => {
    if (ann) {
      setText(ann.text)
      setColor(ann.color || '#94a3b8')
    }
  }, [ann])
  if (!ann) return null
  return (
    <div className="space-y-3 p-3">
      {ann.kind === 'text' && (
        <Field label="Text">
          <textarea className={inputCls} rows={3} value={text} onChange={(e) => setText(e.target.value)} />
        </Field>
      )}
      <Field label="Color">
        <input type="color" value={color} onChange={(e) => setColor(e.target.value)} className="h-8 w-full" />
      </Field>
      {ann.kind !== 'text' && (
        <div className="grid grid-cols-2 gap-2">
          <Field label="Width">
            <input
              type="number"
              className={inputCls}
              defaultValue={ann.width}
              onBlur={(e) => void api.updateAnnotation(lab.id, annId, { width: +e.target.value }).then(refreshLab)}
            />
          </Field>
          <Field label="Height">
            <input
              type="number"
              className={inputCls}
              defaultValue={ann.height}
              onBlur={(e) => void api.updateAnnotation(lab.id, annId, { height: +e.target.value }).then(refreshLab)}
            />
          </Field>
        </div>
      )}
      <button
        onClick={async () => {
          await api.updateAnnotation(lab.id, annId, { text, color })
          await refreshLab()
        }}
        className="w-full rounded-md bg-accent-600 py-1.5 text-sm font-medium text-white hover:bg-accent-500"
      >
        Save
      </button>
      <button
        onClick={async () => {
          await api.deleteAnnotation(lab.id, annId)
          await refreshLab()
        }}
        className="w-full rounded-md bg-red-900/60 py-1.5 text-sm text-red-200 hover:bg-red-800"
      >
        Delete
      </button>
    </div>
  )
}
