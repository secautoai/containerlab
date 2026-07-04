// The Strato workspace: header (brand, breadcrumb, status pill, lab
// controls), agent chat on the left, topology canvas in the middle with a
// floating tool rail, inspector on the right.

import { useEffect, useState, type CSSProperties } from 'react'
import {
  BookOpen,
  Boxes,
  Download,
  Lock,
  LockOpen,
  Network as NetworkIcon,
  Play,
  Share2,
  Square as SquareIcon,
  SquareDashed,
  Type,
} from 'lucide-react'
import { api, type NetworkKind } from '../api'
import { useStore } from '../store'
import { stratoMark } from '../vendors'
import Canvas, { type Selection } from './Canvas'
import Palette from './Palette'
import AgentChat, { toolLabel } from './AgentChat'
import Inspector from './Inspector'
import ShareDialog from './ShareDialog'

const grotesk = "'Space Grotesk', 'IBM Plex Sans', sans-serif"

export default function LabEditor() {
  const lab = useStore((s) => s.lab)
  const system = useStore((s) => s.system)
  const templates = useStore((s) => s.templates)
  const notice = useStore((s) => s.notice)
  const setNotice = useStore((s) => s.setNotice)
  const dismissNotice = useStore((s) => s.dismissNotice)
  const agentBusy = useStore((s) => s.agentBusy)
  // Scalar selectors: agent/tool events and node_state bursts re-render this
  // shell (and the canvas host below it) only when the derived value changes.
  const runningCount = useStore(
    (s) => Object.values(s.states).filter((st) => st === 'running').length,
  )
  const anyRunning = useStore(
    (s) => Object.values(s.states).some((st) => st === 'running' || st === 'starting'),
  )
  const workingOn = useStore((s) => {
    if (!s.agentBusy) return null
    for (let i = s.agentItems.length - 1; i >= 0; i--) {
      const it = s.agentItems[i]
      if (it.kind === 'tool' && it.output === undefined) return toolLabel(it.name, it.input)
    }
    return 'Agent is thinking…'
  })
  const openDashboard = useStore((s) => s.openDashboard)
  const refreshLab = useStore((s) => s.refreshLab)
  const setInspectorTab = useStore((s) => s.setInspectorTab)
  const pushLog = useStore((s) => s.pushLog)
  const [selection, setSelectionRaw] = useState<Selection | null>(null)
  const [netMenu, setNetMenu] = useState(false)
  const [devicesOpen, setDevicesOpen] = useState(false)
  const [docsOpen, setDocsOpen] = useState(false)
  const [docsDraft, setDocsDraft] = useState<string | null>(null)
  const [shareOpen, setShareOpen] = useState(false)
  const [configSets, setConfigSets] = useState<{ active: string; sets: string[] }>({
    active: '',
    sets: [],
  })

  const labId = lab?.id
  useEffect(() => {
    if (!labId) return
    api.configSets(labId).then(setConfigSets).catch(() => {})
  }, [labId])

  // Auto-dismiss the notice after a while (errors linger longer than info).
  useEffect(() => {
    if (!notice) return
    const ms = notice.level === 'info' ? 4000 : 9000
    const t = setTimeout(dismissNotice, ms)
    return () => clearTimeout(t)
  }, [notice, dismissNotice])

  const setSelection = (sel: Selection | null) => {
    setSelectionRaw(sel)
    if (sel) setInspectorTab('details')
    else if (useStore.getState().inspectorTab === 'details') setInspectorTab('console')
  }

  if (!lab) {
    return (
      <div className="flex h-full items-center justify-center text-ink-400">
        <span className="h-5 w-5 animate-spin rounded-full border-2 border-ink-700 border-t-accent-500" />
      </div>
    )
  }

  const nodeCount = Object.keys(lab.nodes).length

  const statusColor = agentBusy ? 'var(--accent)' : anyRunning ? 'var(--green)' : 'var(--muted)'
  const statusLabel = agentBusy ? 'Agent working' : anyRunning ? 'Lab running' : 'Idle'

  // Pre-flight: which nodes can't start on THIS server, and why. netns/
  // container kinds need the bridge datapath (Linux + root); qemu kinds need
  // an uploaded disk image. Computed from data the UI already has so the user
  // learns before clicking Start instead of finding errors buried in Sessions.
  const tmplById = new Map(templates.map((t) => [t.id, t]))
  const blockers: string[] = []
  if (system) {
    let needBridge = 0
    let needImage = 0
    for (const n of Object.values(lab.nodes)) {
      const t = tmplById.get(n.template)
      if (!t) continue
      if (t.kind !== 'qemu' && system.datapath !== 'bridge') needBridge++
      else if (t.kind === 'qemu' && t.available_images.length === 0 && !n.image) needImage++
    }
    if (needBridge > 0)
      blockers.push(
        `${needBridge} ${needBridge === 1 ? 'device needs' : 'devices need'} the bridge datapath (Linux + root) — this server runs the "${system.datapath}" datapath, so ${needBridge === 1 ? 'it' : 'they'} can't boot here`,
      )
    if (needImage > 0)
      blockers.push(`${needImage} ${needImage === 1 ? 'device has' : 'devices have'} no disk image uploaded`)
  }

  const addNetwork = async (kind: NetworkKind) => {
    setNetMenu(false)
    try {
      await api.createNetwork(lab.id, { kind, x: 380 + Math.random() * 80, y: 260 + Math.random() * 80 })
      await refreshLab()
    } catch (e) {
      pushLog('error', `network: ${e instanceof Error ? e.message : e}`)
    }
  }

  const addAnnotation = async (kind: 'text' | 'rect') => {
    try {
      await api.createAnnotation(lab.id, {
        kind,
        x: 300,
        y: 180,
        width: kind === 'rect' ? 220 : 0,
        height: kind === 'rect' ? 140 : 0,
        text: kind === 'text' ? 'Label' : '',
        color: kind === 'text' ? '#8a94a6' : '#2f3846',
        fill: kind === 'rect' ? 'rgba(56,209,186,0.05)' : '',
        font_size: 14,
        z: kind === 'rect' ? -1 : 1,
      })
      await refreshLab()
    } catch (e) {
      pushLog('error', `annotation: ${e instanceof Error ? e.message : e}`)
    }
  }

  const railBtn = (active = false): CSSProperties => ({
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    border: `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
    background: active ? 'var(--accentSoft)' : 'var(--panel)',
    color: active ? 'var(--accent)' : 'var(--muted)',
    borderRadius: 8,
    padding: '6px 10px',
    fontSize: 11.5,
    fontWeight: 600,
    cursor: 'pointer',
    boxShadow: 'var(--shadow)',
    whiteSpace: 'nowrap',
  })

  const headIcon = (active = false): CSSProperties => ({
    display: 'grid',
    placeItems: 'center',
    width: 28,
    height: 28,
    border: `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
    background: active ? 'var(--accentSoft)' : 'transparent',
    color: active ? 'var(--accent)' : 'var(--muted)',
    borderRadius: 8,
    cursor: 'pointer',
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--bg)', color: 'var(--text)', overflow: 'hidden' }}>
      {/* ══ header ══ */}
      <div style={{ height: 50, flex: '0 0 50px', background: 'var(--panel)', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', gap: 12, padding: '0 16px', zIndex: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
          <div style={{ width: 28, height: 28, borderRadius: 8, background: 'linear-gradient(135deg,var(--accent),#2a8fd1)', display: 'grid', placeItems: 'center' }}>
            {stratoMark({ size: 15 })}
          </div>
          <span style={{ fontFamily: grotesk, fontWeight: 700, fontSize: 16, letterSpacing: '-.3px' }}>strato</span>
          <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--accent)', background: 'var(--accentSoft)', borderRadius: 5, padding: '2px 7px', letterSpacing: '.4px', whiteSpace: 'nowrap' }}>
            AI NETWORK ENGINEER
          </span>
        </div>
        <div style={{ width: 1, height: 20, background: 'var(--border)' }} />
        <button
          onClick={openDashboard}
          className="btn-ghost"
          style={{ border: 'none', background: 'transparent', fontSize: 13, fontWeight: 500, color: 'var(--muted)', cursor: 'pointer', padding: 0 }}
          title="All labs"
        >
          Labs /
        </button>
        <span style={{ fontSize: 13, fontWeight: 600, marginLeft: -6, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: 200 }}>
          {lab.name}
        </span>
        <span style={{ fontSize: 11, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
          {nodeCount} devices · {runningCount} running
        </span>
        {system && (
          <span style={{ fontSize: 11, color: 'var(--muted)', background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 999, padding: '3px 10px', whiteSpace: 'nowrap' }}>
            {lab.kvm ? 'kvm' : 'tcg'} · {system.datapath}
          </span>
        )}
        {!lab.kvm && (
          <span title="No /dev/kvm — VM nodes run under slow TCG emulation" style={{ fontSize: 10, color: 'var(--amber)', background: 'rgba(232,179,72,.1)', borderRadius: 5, padding: '2px 6px', whiteSpace: 'nowrap' }}>
            no KVM
          </span>
        )}
        <div style={{ flex: 1 }} />

        <div style={{ display: 'flex', alignItems: 'center', gap: 7, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 999, padding: '5px 12px' }}>
          <span style={{ width: 7, height: 7, borderRadius: '50%', background: statusColor, animation: agentBusy ? 'pulseDot 1s ease-in-out infinite' : 'none' }} />
          <span style={{ fontSize: 12, fontWeight: 500, color: statusColor, whiteSpace: 'nowrap' }}>{statusLabel}</span>
        </div>

        {!anyRunning ? (
          <button
            onClick={async () => {
              if (nodeCount === 0) {
                setNotice('warn', 'This lab has no devices yet — add some before starting.')
                return
              }
              if (blockers.length) {
                // Everything is blocked: don't fire a start that will only
                // produce a wall of Sessions errors; explain instead.
                setNotice('warn', `Can't start on this server — ${blockers.join('; ')}.`)
                return
              }
              pushLog('info', 'starting lab…')
              try {
                await api.startLab(lab.id)
              } catch (e) {
                setNotice('error', `start: ${e instanceof Error ? e.message : e}`)
              }
            }}
            title={blockers.length ? blockers.join('; ') : 'Boot every device in this lab'}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              border: blockers.length ? '1px solid var(--amber)' : 'none',
              background: blockers.length ? 'rgba(232,179,72,.15)' : 'var(--accent)',
              color: blockers.length ? 'var(--amber)' : '#08211d',
              borderRadius: 9,
              padding: '7px 13px',
              fontSize: 12.5,
              fontWeight: 600,
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            <Play size={13} /> Start lab
          </button>
        ) : (
          <button
            onClick={async () => {
              try {
                await api.stopLab(lab.id)
                await refreshLab()
              } catch (e) {
                pushLog('error', `stop: ${e instanceof Error ? e.message : e}`)
              }
            }}
            className="btn-danger"
            style={{ display: 'flex', alignItems: 'center', gap: 6, border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 9, padding: '7px 13px', fontSize: 12.5, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap' }}
          >
            <SquareIcon size={13} /> Stop lab
          </button>
        )}

        <select
          title="Active configuration set (boot configs)"
          value={configSets.active}
          onChange={async (e) => {
            try {
              if (e.target.value === '__snapshot__') {
                const name = prompt('Snapshot current startup configs as set:')
                if (name?.trim()) {
                  const v = await api.snapshotConfigSet(lab.id, name.trim())
                  setConfigSets(v)
                  pushLog('info', `config set '${name.trim()}' saved`)
                }
                return
              }
              const v = await api.activateConfigSet(lab.id, e.target.value)
              setConfigSets(v)
              pushLog('info', v.active ? `booting from config set '${v.active}'` : 'booting from default configs')
            } catch (err) {
              pushLog('error', `config set: ${err instanceof Error ? err.message : err}`)
            }
          }}
          style={{ background: 'var(--panel2)', border: '1px solid var(--border)', color: 'var(--muted)', borderRadius: 8, padding: '5px 6px', fontSize: 11, outline: 'none', maxWidth: 130 }}
        >
          <option value="">configs: default</option>
          {configSets.sets.map((s) => (
            <option key={s} value={s}>
              configs: {s}
            </option>
          ))}
          <option value="__snapshot__">+ snapshot as set…</option>
        </select>

        <button
          onClick={async () => {
            try {
              await api.setLock(lab.id, !lab.locked)
              await refreshLab()
              pushLog('info', lab.locked ? 'lab unlocked' : 'lab locked (read-only)')
            } catch (e) {
              pushLog('error', `lock: ${e instanceof Error ? e.message : e}`)
            }
          }}
          title={lab.locked ? 'Unlock lab (allow edits)' : 'Lock lab (read-only)'}
          className="iconbtn"
          style={headIcon(!!lab.locked)}
        >
          {lab.locked ? <Lock size={13} /> : <LockOpen size={13} />}
        </button>
        <button
          onClick={() => {
            // Seed the draft only when opening; closing keeps unsaved edits.
            if (!docsOpen) setDocsDraft(lab.body ?? '')
            setDocsOpen(!docsOpen)
          }}
          title="Lab documentation"
          className="iconbtn"
          style={headIcon(docsOpen)}
        >
          <BookOpen size={13} />
        </button>
        {system?.auth_enabled && (
          <button
            onClick={() => setShareOpen(true)}
            title="Share lab"
            className="iconbtn"
            style={headIcon(shareOpen)}
          >
            <Share2 size={13} />
          </button>
        )}
        <a href={api.exportUrl(lab.id)} title="Export lab" className="iconbtn" style={headIcon()}>
          <Download size={13} />
        </a>
      </div>

      {/* ══ body ══ */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden', minHeight: 0 }}>
        <AgentChat />

        <div style={{ flex: '1 1 auto', position: 'relative', overflow: 'hidden', minWidth: 260 }}>
          <Canvas onSelect={setSelection} />

          {/* floating tool rail */}
          <div className="absolute left-3.5 top-3.5 z-20 flex gap-2">
            <button className="chip" style={railBtn(devicesOpen)} onClick={() => setDevicesOpen(!devicesOpen)}>
              <Boxes size={13} /> Devices
            </button>
            <div style={{ position: 'relative' }}>
              <button className="chip" style={railBtn(netMenu)} onClick={() => setNetMenu(!netMenu)} title="Add network segment">
                <NetworkIcon size={13} /> Network
              </button>
              {netMenu && (
                <div style={{ position: 'absolute', left: 0, top: '100%', marginTop: 6, width: 176, overflow: 'hidden', borderRadius: 10, border: '1px solid var(--border2)', background: 'var(--panel)', boxShadow: 'var(--shadow)', zIndex: 50, padding: '4px 0' }}>
                  {(['bridge', 'nat', 'management', 'cloud'] as NetworkKind[]).map((k) => (
                    <button
                      key={k}
                      onClick={() => void addNetwork(k)}
                      className="block w-full px-3 py-1.5 text-left text-sm text-ink-200 hover:bg-ink-850"
                      style={{ border: 'none', background: 'transparent', cursor: 'pointer' }}
                    >
                      {k}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <button className="chip" style={railBtn()} onClick={() => void addAnnotation('text')} title="Add text label">
              <Type size={13} />
            </button>
            <button className="chip" style={railBtn()} onClick={() => void addAnnotation('rect')} title="Add region">
              <SquareDashed size={13} />
            </button>
          </div>

          {/* device palette overlay */}
          {devicesOpen && (
            <div className="absolute inset-y-0 left-0 z-30" style={{ boxShadow: 'var(--shadow)' }}>
              <Palette onClose={() => setDevicesOpen(false)} />
            </div>
          )}

          {/* pre-flight banner: this lab can't fully start on this server */}
          {!anyRunning && blockers.length > 0 && (
            <div
              style={{
                position: 'absolute',
                top: 56,
                left: '50%',
                transform: 'translateX(-50%)',
                background: 'rgba(232,179,72,.1)',
                border: '1px solid rgba(232,179,72,.4)',
                borderRadius: 12,
                padding: '9px 14px',
                fontSize: 12,
                color: 'var(--amber)',
                boxShadow: 'var(--shadow)',
                zIndex: 14,
                maxWidth: '76%',
                lineHeight: 1.5,
              }}
            >
              <strong style={{ fontWeight: 700 }}>Heads up:</strong>{' '}
              {blockers.join('; ')}. Run this lab on a Linux host with{' '}
              <code style={{ fontFamily: "'IBM Plex Mono',monospace" }}>--datapath bridge</code>, or
              use QEMU devices (Linux/OpenWrt) which boot here.
            </div>
          )}

          {/* transient notice: start failures & server errors, dismissible */}
          {notice && (
            <div
              onClick={dismissNotice}
              title="Dismiss"
              style={{
                position: 'absolute',
                bottom: 52,
                left: '50%',
                transform: 'translateX(-50%)',
                background: 'var(--panel)',
                border: `1px solid ${notice.level === 'error' ? 'var(--red)' : notice.level === 'warn' ? 'var(--amber)' : 'var(--border2)'}`,
                borderRadius: 12,
                padding: '10px 15px',
                fontSize: 12.5,
                color: notice.level === 'error' ? 'var(--red)' : notice.level === 'warn' ? 'var(--amber)' : 'var(--text)',
                boxShadow: 'var(--shadow)',
                zIndex: 16,
                maxWidth: '80%',
                lineHeight: 1.5,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'flex-start',
                gap: 9,
                animation: 'fadeUp .2s ease',
              }}
            >
              <span style={{ marginTop: 1 }}>{notice.level === 'error' ? '⚠' : notice.level === 'warn' ? '⚠' : 'ℹ'}</span>
              <span style={{ flex: 1 }}>{notice.text}</span>
              <span style={{ opacity: 0.6, fontSize: 11 }}>✕</span>
            </div>
          )}

          {/* agent toast */}
          {workingOn && (
            <div style={{ position: 'absolute', top: 14, left: '50%', transform: 'translateX(-50%)', background: 'var(--panel)', border: '1px solid var(--border2)', borderRadius: 999, padding: '7px 16px', fontSize: 12, color: 'var(--muted)', boxShadow: 'var(--shadow)', zIndex: 15, display: 'flex', alignItems: 'center', gap: 8, animation: 'fadeUp .25s ease', maxWidth: '70%', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
              <span style={{ width: 11, height: 11, flex: '0 0 11px', borderRadius: '50%', border: '2px solid var(--accentSoft)', borderTopColor: 'var(--accent)', animation: 'spinDot .7s linear infinite' }} />
              {workingOn}
            </div>
          )}
        </div>

        {docsOpen && (
          <aside style={{ width: 340, flex: '0 0 340px', background: 'var(--panel)', borderLeft: '1px solid var(--border)', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, borderBottom: '1px solid var(--border)', padding: '10px 12px' }}>
              <BookOpen size={14} style={{ color: 'var(--accent)' }} />
              <h3 style={{ fontSize: 13, fontWeight: 600, margin: 0 }}>Lab documentation</h3>
              <button
                onClick={async () => {
                  try {
                    await api.updateLab(lab.id, { body: docsDraft ?? '' })
                    await refreshLab()
                    pushLog('info', 'lab documentation saved')
                  } catch (e) {
                    pushLog('error', `docs: ${e instanceof Error ? e.message : e}`)
                  }
                }}
                style={{ marginLeft: 'auto', border: 'none', background: 'var(--accent)', color: '#08211d', borderRadius: 7, padding: '4px 10px', fontSize: 11, fontWeight: 600, cursor: 'pointer' }}
              >
                Save
              </button>
            </div>
            <textarea
              value={docsDraft ?? lab.body ?? ''}
              onChange={(e) => setDocsDraft(e.target.value)}
              placeholder={'# Lab workbook\n\nGoals, addressing plan, tasks…  (Markdown)'}
              style={{ minHeight: 0, flex: 1, resize: 'none', background: '#0a0c0f', border: 'none', outline: 'none', padding: 12, fontFamily: "'IBM Plex Mono', monospace", fontSize: 11.5, lineHeight: 1.6, color: 'var(--text)' }}
            />
          </aside>
        )}

        <Inspector selection={selection} onClearSelection={() => setSelection(null)} />
      </div>

      {shareOpen && system?.auth_enabled && (
        <ShareDialog labId={lab.id} onClose={() => setShareOpen(false)} />
      )}
    </div>
  )
}
