// Strato inspector: the right pane. Console (real xterm/VNC sessions),
// Configs (startup configs straight from the lab document), Validation
// (agent-reported checks), Sessions (the full audit log), and Details
// (properties of the current canvas selection).

import { useEffect, useMemo, useRef } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { api, wsUrl } from '../api'
import { useStore, type InspectorTab, type SessionKind } from '../store'
import type { Selection } from './Canvas'
import SidePanel from './SidePanel'
import VncViewer from './VncViewer'

const mono = "'IBM Plex Mono', ui-monospace, monospace"

// ---------- real terminal ----------

function XTerm({ labId, nodeId }: { labId: string; nodeId: string }) {
  const holder = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = holder.current
    if (!el) return
    const term = new Terminal({
      fontFamily: mono,
      fontSize: 12.5,
      cursorBlink: true,
      theme: {
        background: '#0a0c0f',
        foreground: '#cde5d8',
        cursor: '#38d1ba',
        selectionBackground: '#2f3846',
      },
    })
    const fit = new FitAddon()
    term.loadAddon(fit)
    term.open(el)
    fit.fit()

    const ws = new WebSocket(wsUrl(`/api/ws/console/${labId}/${nodeId}`))
    ws.binaryType = 'arraybuffer'
    ws.onopen = () => term.focus()
    ws.onmessage = (ev) => {
      if (typeof ev.data === 'string') term.write(ev.data)
      else term.write(new Uint8Array(ev.data))
    }
    ws.onclose = () => term.write('\r\n\x1b[90m[disconnected]\x1b[0m\r\n')
    const sub = term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(data)
    })

    const resize = new ResizeObserver(() => fit.fit())
    resize.observe(el)

    return () => {
      resize.disconnect()
      sub.dispose()
      ws.close()
      term.dispose()
    }
  }, [labId, nodeId])

  return <div ref={holder} className="h-full w-full" style={{ background: '#0a0c0f', padding: '6px 0 0 8px' }} />
}

// ---------- small shared bits ----------

function NodeChip({
  label,
  active,
  disabled,
  onClick,
  onClose,
  title,
}: {
  label: string
  active?: boolean
  disabled?: boolean
  onClick: () => void
  onClose?: () => void
  title?: string
}) {
  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 5,
        border: `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
        background: active ? 'var(--accentSoft)' : 'var(--panel2)',
        color: disabled ? 'var(--border2)' : active ? 'var(--accent)' : 'var(--muted)',
        borderRadius: 7,
        padding: '4px 9px',
        fontSize: 11,
        fontWeight: 600,
        fontFamily: mono,
        cursor: disabled ? 'default' : 'pointer',
        whiteSpace: 'nowrap',
      }}
      title={title}
      onClick={() => {
        if (!disabled) onClick()
      }}
    >
      {label}
      {onClose && (
        <span
          onClick={(e) => {
            e.stopPropagation()
            onClose()
          }}
          style={{ cursor: 'pointer', opacity: 0.7, marginRight: -3 }}
          title="Close session"
        >
          ✕
        </span>
      )}
    </span>
  )
}

function EmptyPane({ text }: { text: string }) {
  return (
    <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>
      {text}
    </div>
  )
}

// ---------- console tab ----------

function ConsolePane() {
  const lab = useStore((s) => s.lab)
  const states = useStore((s) => s.states)
  const consoles = useStore((s) => s.consoles)
  const active = useStore((s) => s.activeConsole)
  const setActive = useStore((s) => s.setActiveConsole)
  const openConsole = useStore((s) => s.openConsole)
  const closeConsole = useStore((s) => s.closeConsole)
  if (!lab) return null

  const nodes = Object.values(lab.nodes).sort((a, b) => a.name.localeCompare(b.name))
  const activeNode = active ? lab.nodes[active] : null
  const open = new Set(consoles.map((c) => c.nodeId))

  if (!nodes.length) {
    return <EmptyPane text="No devices yet — the consoles of every deployed device open here." />
  }

  return (
    <>
      <div style={{ display: 'flex', gap: 6, padding: '10px 12px 0', flexWrap: 'wrap', flex: '0 0 auto' }}>
        {nodes.map((n) => {
          const st = states[n.id] ?? 'stopped'
          const up = st === 'running' || st === 'starting'
          return (
            <NodeChip
              key={n.id}
              label={n.name}
              active={active === n.id}
              disabled={!up && !open.has(n.id)}
              title={up ? `console ${n.name}` : `${n.name} is ${st}`}
              onClick={() => (open.has(n.id) ? setActive(n.id) : openConsole(n.id, n.name))}
              onClose={open.has(n.id) ? () => closeConsole(n.id) : undefined}
            />
          )
        })}
      </div>
      {consoles.length > 0 ? (
        <>
          {activeNode && (
            <div style={{ fontSize: 11, color: 'var(--muted)', padding: '9px 14px 7px', fontFamily: mono, flex: '0 0 auto' }}>
              console {activeNode.name} · {activeNode.template}
              {activeNode.console === 'vnc' ? ' · vnc' : ''}
            </div>
          )}
          <div style={{ flex: 1, minHeight: 0, borderTop: '1px solid var(--border)', background: '#0a0c0f' }}>
            {consoles.map((c) => (
              <div key={c.nodeId} className="h-full" style={{ display: active === c.nodeId ? 'block' : 'none' }}>
                {lab.nodes[c.nodeId]?.console === 'vnc' ? (
                  <VncViewer labId={lab.id} nodeId={c.nodeId} />
                ) : (
                  <XTerm labId={lab.id} nodeId={c.nodeId} />
                )}
              </div>
            ))}
          </div>
        </>
      ) : (
        <EmptyPane text="No session — click a running device on the canvas (or a chip above) to open its real console." />
      )}
    </>
  )
}

// ---------- configs tab ----------

function ConfigsPane() {
  const lab = useStore((s) => s.lab)
  const states = useStore((s) => s.states)
  const cfgTouched = useStore((s) => s.cfgTouched)
  const inspectorNode = useStore((s) => s.inspectorNode)
  const setInspectorNode = useStore((s) => s.setInspectorNode)
  const refreshLab = useStore((s) => s.refreshLab)
  const pushLog = useStore((s) => s.pushLog)
  if (!lab) return null

  const nodes = Object.values(lab.nodes).sort((a, b) => a.name.localeCompare(b.name))
  const sel = (inspectorNode && lab.nodes[inspectorNode] ? inspectorNode : null) ?? nodes[0]?.id ?? null
  const node = sel ? lab.nodes[sel] : null
  const lines = node?.startup_config ? node.startup_config.split('\n') : []
  const state = node ? states[node.id] ?? 'stopped' : 'stopped'

  if (!nodes.length) {
    return <EmptyPane text="No configs yet — the agent writes per-vendor configurations when it designs a lab." />
  }

  return (
    <>
      <div style={{ display: 'flex', gap: 6, padding: '10px 12px', flexWrap: 'wrap', flex: '0 0 auto' }}>
        {nodes.map((n) => (
          <NodeChip key={n.id} label={n.name} active={sel === n.id} onClick={() => setInspectorNode(n.id)} />
        ))}
      </div>
      {node && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 14px 8px', flex: '0 0 auto' }}>
          <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: mono }}>
            {node.name} · startup config
          </span>
          <span style={{ flex: 1 }} />
          {!!cfgTouched[node.name] && (
            <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--amber)', background: 'rgba(232,179,72,.12)', borderRadius: 5, padding: '2px 7px' }}>
              modified by agent
            </span>
          )}
          {state === 'running' && (
            <>
              <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--green)', background: 'rgba(62,207,142,.1)', borderRadius: 5, padding: '2px 7px' }}>
                running
              </span>
              <button
                className="btn-ghost"
                title="Pull the live running configuration off the console into the startup config"
                onClick={async () => {
                  try {
                    pushLog('info', `${node.name}: exporting running config…`)
                    await api.exportConfig(lab.id, node.id)
                    await refreshLab()
                    pushLog('info', `${node.name}: running config exported to startup config`)
                  } catch (e) {
                    pushLog('error', `export: ${e instanceof Error ? e.message : e}`)
                  }
                }}
                style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 7, padding: '3px 9px', fontSize: 10.5, fontWeight: 600, cursor: 'pointer' }}
              >
                Pull running
              </button>
            </>
          )}
        </div>
      )}
      <div style={{ flex: 1, overflow: 'auto', background: '#0a0c0f', borderTop: '1px solid var(--border)', padding: '10px 0', fontFamily: mono, fontSize: 11.5, lineHeight: 1.6, userSelect: 'text' }}>
        {lines.length === 0 && (
          <div style={{ color: 'var(--muted)', padding: '10px 14px', fontFamily: "'IBM Plex Sans', sans-serif", fontSize: 12 }}>
            No startup config — ask the agent to write one, or edit it in the node's Details.
          </div>
        )}
        {lines.map((l, i) => (
          <div key={i} style={{ display: 'flex' }}>
            <span style={{ width: 38, flex: '0 0 38px', textAlign: 'right', paddingRight: 10, color: '#3d4756' }}>{i + 1}</span>
            <span style={{ whiteSpace: 'pre', color: '#cdd6e4' }}>{l}</span>
          </div>
        ))}
      </div>
    </>
  )
}

// ---------- validation tab ----------

function ValidationPane() {
  const checks = useStore((s) => s.checks)
  const lastValidated = useStore((s) => s.lastValidated)
  const agentBusy = useStore((s) => s.agentBusy)
  const agentConnected = useStore((s) => s.agentConnected)
  const sendAgent = useStore((s) => s.sendAgent)

  const passed = checks.filter((c) => c.status === 'pass').length
  const warn = checks.filter((c) => c.status === 'warn').length
  const fail = checks.filter((c) => c.status === 'fail').length
  const bad = fail > 0
  const fg = bad ? 'var(--red)' : warn ? 'var(--amber)' : 'var(--green)'
  const when = lastValidated
    ? new Date(lastValidated).toTimeString().slice(0, 8)
    : null

  if (!checks.length) {
    return (
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 12, color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>
        <div>No validation run yet — the agent verifies labs on the real device consoles and reports each check here.</div>
        <button
          className="btn-ghost"
          disabled={agentBusy || !agentConnected}
          onClick={() =>
            sendAgent(
              'Validate the current lab: check protocol adjacencies, expected routes, and end-to-end reachability with run_command, and report each result with report_check.',
            )
          }
          style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 8, padding: '6px 12px', fontSize: 11.5, fontWeight: 600, cursor: 'pointer' }}
        >
          Run validation
        </button>
      </div>
    )
  }

  return (
    <>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 16px 10px', flex: '0 0 auto' }}>
        <div style={{ width: 38, height: 38, borderRadius: 11, background: bad ? 'rgba(240,101,90,.12)' : warn ? 'rgba(232,179,72,.13)' : 'rgba(62,207,142,.12)', display: 'grid', placeItems: 'center' }}>
          <svg width={19} height={19} viewBox="0 0 24 24" fill="none" stroke={fg} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" />
          </svg>
        </div>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 13.5, fontWeight: 600 }}>
            {bad || warn
              ? `${passed}/${checks.length} passed${warn ? ` · ${warn} warning${warn > 1 ? 's' : ''}` : ''}${fail ? ` · ${fail} failed` : ''}`
              : `All checks passing (${passed}/${checks.length})`}
          </div>
          <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 1 }}>
            {when ? `Last run ${when} · on real device consoles` : 'Checks are streaming in'}
          </div>
        </div>
        <button
          className="btn-ghost"
          disabled={agentBusy || !agentConnected}
          onClick={() =>
            sendAgent(
              'Re-run validation on the current lab: check protocol adjacencies, expected routes, and end-to-end reachability with run_command, and report each result with report_check.',
            )
          }
          style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 8, padding: '6px 11px', fontSize: 11.5, fontWeight: 600, cursor: 'pointer', opacity: agentBusy ? 0.6 : 1 }}
        >
          Re-run
        </button>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '4px 14px 16px', display: 'flex', flexDirection: 'column', gap: 7 }}>
        {checks.map((c, ci) => (
          <div key={ci} style={{ background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 10, padding: '10px 12px', animation: 'fadeUp .25s ease' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ width: 16, height: 16, flex: '0 0 16px', borderRadius: '50%', background: c.status === 'pass' ? 'rgba(62,207,142,.15)' : c.status === 'warn' ? 'rgba(232,179,72,.15)' : 'rgba(240,101,90,.15)', display: 'grid', placeItems: 'center' }}>
                <svg width={9} height={9} viewBox="0 0 24 24" fill="none" stroke={c.status === 'pass' ? 'var(--green)' : c.status === 'warn' ? 'var(--amber)' : 'var(--red)'} strokeWidth={3.2} strokeLinecap="round" strokeLinejoin="round">
                  <path d={c.status === 'pass' ? 'M4 12.5 9.5 18 20 6.5' : c.status === 'warn' ? 'M12 5v9M12 18.5v.01' : 'M5 5l14 14M19 5 5 19'} />
                </svg>
              </span>
              <span style={{ fontSize: 12.5, fontWeight: 600 }}>{c.label}</span>
              <span style={{ marginLeft: 'auto', fontSize: 10.5, fontWeight: 700, color: c.status === 'pass' ? 'var(--green)' : c.status === 'warn' ? 'var(--amber)' : 'var(--red)', letterSpacing: '.5px' }}>
                {c.status.toUpperCase()}
              </span>
            </div>
            {!!c.detail && (
              <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 5, marginLeft: 24, fontFamily: mono, userSelect: 'text', overflowWrap: 'anywhere' }}>
                {c.detail}
              </div>
            )}
          </div>
        ))}
        {agentBusy && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 2px', color: 'var(--muted)', fontSize: 12 }}>
            <span style={{ width: 11, height: 11, borderRadius: '50%', border: '2px solid var(--accentSoft)', borderTopColor: 'var(--accent)', animation: 'spinDot .7s linear infinite' }} />
            agent is working…
          </div>
        )}
      </div>
    </>
  )
}

// ---------- sessions tab ----------

const sessionKindStyle = (kind: SessionKind) =>
  ({
    prompt: { kind: 'PROMPT', fg: 'var(--accent)', bg: 'var(--accentSoft)' },
    agent: { kind: 'AGENT', fg: 'var(--violet)', bg: 'rgba(167,139,250,.14)' },
    step: { kind: 'STEP', fg: 'var(--muted)', bg: 'var(--panel)' },
    deploy: { kind: 'DEPLOY', fg: 'var(--green)', bg: 'rgba(62,207,142,.12)' },
    check: { kind: 'CHECK', fg: 'var(--amber)', bg: 'rgba(232,179,72,.12)' },
    ssh: { kind: 'SSH', fg: '#5a7ff0', bg: 'rgba(90,127,240,.13)' },
    cli: { kind: 'CLI', fg: '#5a7ff0', bg: 'rgba(90,127,240,.13)' },
    error: { kind: 'ERROR', fg: 'var(--red)', bg: 'rgba(240,101,90,.12)' },
    warn: { kind: 'WARN', fg: 'var(--amber)', bg: 'rgba(232,179,72,.12)' },
    sys: { kind: 'SYS', fg: 'var(--muted)', bg: 'var(--panel)' },
  })[kind] ?? { kind: String(kind).toUpperCase(), fg: 'var(--muted)', bg: 'var(--panel)' }

function SessionsPane() {
  const sessions = useStore((s) => s.sessions)
  const clearSessions = useStore((s) => s.clearSessions)
  const labName = useStore((s) => s.lab?.name ?? 'lab')
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    endRef.current?.scrollIntoView()
  }, [sessions]) // array identity changes on every append, even at the cap

  const exportSessions = () => {
    const txt = sessions
      .map((e) => `${new Date(e.ts).toISOString()}  [${e.kind.toUpperCase()}]  ${e.text}`)
      .join('\n')
    const blob = new Blob([txt], { type: 'text/plain' })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `${labName}-session.log`
    a.click()
    URL.revokeObjectURL(a.href)
  }

  if (!sessions.length) {
    return <EmptyPane text="Nothing recorded yet — every prompt, agent action, deploy, and console command lands here." />
  }

  return (
    <>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '12px 14px 8px', flex: '0 0 auto' }}>
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Everything in this lab session — prompts, agent actions, deploys, console activity.
        </span>
        <span style={{ flex: 1 }} />
        <button
          className="btn-ghost"
          onClick={exportSessions}
          style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 8, padding: '5px 10px', fontSize: 11, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap' }}
        >
          Export .log
        </button>
        <button
          className="btn-danger"
          onClick={clearSessions}
          style={{ border: '1px solid var(--border)', background: 'transparent', color: 'var(--muted)', borderRadius: 8, padding: '5px 10px', fontSize: 11, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap' }}
        >
          Clear
        </button>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '4px 12px 14px', display: 'flex', flexDirection: 'column', gap: 5 }}>
        {sessions.map((e, ei) => {
          const K = sessionKindStyle(e.kind)
          return (
            <div key={ei} style={{ display: 'flex', gap: 9, alignItems: 'flex-start', background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 9, padding: '8px 10px' }}>
              <span style={{ fontSize: 10, color: 'var(--muted)', fontFamily: mono, marginTop: 3, whiteSpace: 'nowrap' }}>
                {new Date(e.ts).toTimeString().slice(0, 8)}
              </span>
              <span style={{ fontSize: 9, fontWeight: 700, letterSpacing: '.5px', color: K.fg, background: K.bg, borderRadius: 4, padding: '2px 6px', marginTop: 1, whiteSpace: 'nowrap' }}>
                {K.kind}
              </span>
              <span style={{ fontSize: 11.5, lineHeight: 1.5, color: 'var(--text)', userSelect: 'text', minWidth: 0, overflowWrap: 'anywhere' }}>
                {e.text}
              </span>
            </div>
          )
        })}
        <div ref={endRef} />
      </div>
    </>
  )
}

// ---------- the inspector ----------

export default function Inspector({
  selection,
  onClearSelection,
}: {
  selection: Selection | null
  onClearSelection: () => void
}) {
  const tab = useStore((s) => s.inspectorTab)
  const setTab = useStore((s) => s.setInspectorTab)
  const lab = useStore((s) => s.lab)
  // Badges subscribe to counts, not arrays — a session/check append must not
  // re-render the whole inspector (and the open xterm) for a number change.
  const consoleCount = useStore((s) => s.consoles.length)
  const checkCount = useStore((s) => s.checks.length)
  const sessionCount = useStore((s) => s.sessions.length)

  const configCount = useMemo(
    () => Object.values(lab?.nodes ?? {}).filter((n) => !!n.startup_config).length,
    [lab],
  )

  const tabs: { key: InspectorTab; label: string; badge: number }[] = [
    { key: 'console', label: 'Console', badge: consoleCount },
    { key: 'configs', label: 'Configs', badge: configCount },
    { key: 'validation', label: 'Validation', badge: checkCount },
    { key: 'sessions', label: 'Sessions', badge: sessionCount },
  ]
  if (selection) tabs.push({ key: 'details', label: 'Details', badge: 0 })
  const shown: InspectorTab = tab === 'details' && !selection ? 'console' : tab

  return (
    <div
      style={{
        width: 400,
        flex: '0 1 400px',
        minWidth: 280,
        background: 'var(--panel)',
        borderLeft: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
        minHeight: 0,
      }}
    >
      <div style={{ display: 'flex', borderBottom: '1px solid var(--border)', padding: '0 4px', flex: '0 0 auto', minWidth: 0 }}>
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            style={{
              border: 'none',
              background: 'transparent',
              color: shown === t.key ? 'var(--text)' : 'var(--muted)',
              fontSize: 12,
              fontWeight: 600,
              padding: '13px 8px 11px',
              cursor: 'pointer',
              borderBottom: `2px solid ${shown === t.key ? 'var(--accent)' : 'transparent'}`,
              display: 'flex',
              alignItems: 'center',
              gap: 5,
              minWidth: 0,
              flex: '0 1 auto',
              whiteSpace: 'nowrap',
              overflow: 'hidden',
            }}
          >
            {t.label}
            {t.badge > 0 && (
              <span style={{ fontSize: 10, fontWeight: 600, background: shown === t.key ? 'var(--accentSoft)' : 'var(--panel2)', color: shown === t.key ? 'var(--accent)' : 'var(--muted)', borderRadius: 999, padding: '1px 6px' }}>
                {t.badge}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* ConsolePane stays mounted across tab switches so xterm/VNC
          sessions (and their scrollback) survive canvas selections. */}
      <div style={{ display: shown === 'console' ? 'contents' : 'none' }}>
        <ConsolePane />
      </div>
      {shown === 'configs' && <ConfigsPane />}
      {shown === 'validation' && <ValidationPane />}
      {shown === 'sessions' && <SessionsPane />}
      {shown === 'details' && selection && (
        <div className="min-h-0 flex-1 overflow-y-auto">
          <SidePanel selection={selection} onClose={onClearSelection} />
        </div>
      )}
    </div>
  )
}
