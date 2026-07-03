// Strato agent chat: the left pane. User prompts, agent prose, a live
// step timeline for every tool call (expandable to raw input/output),
// validation report cards, suggestion chips, and the composer — all
// driven by the real /api/ws/agent event stream in the store.

import { useEffect, useMemo, useRef, useState } from 'react'
import { normalizeCheckStatus, useStore, type ChatItem, type CheckStatus } from '../store'
import { stratoMark } from '../vendors'

const mono = "'IBM Plex Mono', ui-monospace, monospace"
const grotesk = "'Space Grotesk', 'IBM Plex Sans', sans-serif"

interface ToolInput {
  name?: string
  template?: string
  node?: string
  a_node?: string
  b_node?: string
  network?: string
  command?: string
  label?: string
  status?: string
  detail?: string
  suspended?: boolean
}

export function toolLabel(name: string, rawInput: unknown): string {
  const i = (rawInput ?? {}) as ToolInput
  switch (name) {
    case 'get_lab':
      return 'Reading the lab topology'
    case 'list_templates':
      return 'Checking the device catalog'
    case 'create_node':
      return `Deploying ${i.name ?? 'node'} (${i.template ?? '?'})`
    case 'update_node':
      return `Updating ${i.name ?? 'node'}`
    case 'delete_node':
      return `Removing ${i.name ?? 'node'}`
    case 'create_link':
      return `Wiring ${i.a_node ?? '?'} ↔ ${i.b_node ?? i.network ?? '?'}`
    case 'create_network':
      return `Creating network ${i.name ?? ''}`
    case 'set_startup_config':
      return `Writing config for ${i.node ?? 'node'}`
    case 'start':
      return `Starting ${i.node ?? 'the lab'}`
    case 'stop':
      return `Stopping ${i.node ?? 'the lab'}`
    case 'run_command':
      return `${i.node ?? '?'} $ ${(i.command ?? '').slice(0, 44)}`
    case 'set_link_quality':
      return i.suspended
        ? `Failing link ${i.a_node ?? '?'} ↔ ${i.b_node ?? '?'}`
        : `Tuning link ${i.a_node ?? '?'} ↔ ${i.b_node ?? '?'}`
    case 'report_check':
      return `Recording check: ${i.label ?? '?'}`
    default:
      return name
  }
}

function toolMeta(item: Extract<ChatItem, { kind: 'tool' }>): string {
  if (item.output === undefined) return ''
  if (item.isError) return 'failed'
  switch (item.name) {
    case 'run_command': {
      const lines = item.output.split('\n').filter((l) => l.trim()).length
      return `${lines} line${lines === 1 ? '' : 's'}`
    }
    case 'create_node':
      return 'deployed'
    case 'create_link':
      return 'wired'
    case 'set_startup_config':
      return 'written'
    case 'get_lab':
    case 'list_templates':
      return 'ok'
    default:
      return 'done'
  }
}

// Group the flat item stream into user bubbles and agent runs whose
// consecutive tool calls collapse into one Strato step card.
type RunPart =
  | { kind: 'text'; text: string }
  | { kind: 'steps'; tools: Extract<ChatItem, { kind: 'tool' }>[] }
  | { kind: 'report'; passed: number; warn: number; fail: number; total: number }
  | { kind: 'error'; text: string }

type Block = { kind: 'user'; text: string; files?: string[] } | { kind: 'run'; parts: RunPart[] }

function groupItems(items: ChatItem[]): Block[] {
  const blocks: Block[] = []
  for (const it of items) {
    if (it.kind === 'user') {
      blocks.push({ kind: 'user', text: it.text, files: it.files })
      continue
    }
    let run = blocks[blocks.length - 1]
    if (!run || run.kind !== 'run') {
      run = { kind: 'run', parts: [] }
      blocks.push(run)
    }
    const parts = run.parts
    if (it.kind === 'assistant') {
      parts.push({ kind: 'text', text: it.text })
    } else if (it.kind === 'tool') {
      const last = parts[parts.length - 1]
      if (last && last.kind === 'steps') last.tools.push(it)
      else parts.push({ kind: 'steps', tools: [it] })
    } else if (it.kind === 'report') {
      parts.push({ kind: 'report', passed: it.passed, warn: it.warn, fail: it.fail, total: it.total })
    } else if (it.kind === 'error') {
      parts.push({ kind: 'error', text: it.text })
    }
  }
  return blocks
}

const checkColors: Record<CheckStatus, string> = {
  pass: 'var(--green)',
  warn: 'var(--amber)',
  fail: 'var(--red)',
}

function StepRow({ tool }: { tool: Extract<ChatItem, { kind: 'tool' }> }) {
  const [open, setOpen] = useState(false)
  const pending = tool.output === undefined

  // report_check renders as a Strato check row, not a working step.
  if (tool.name === 'report_check') {
    const i = (tool.input ?? {}) as ToolInput
    const st = normalizeCheckStatus(i.status)
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
        <span
          style={{
            width: 13,
            height: 13,
            flex: '0 0 13px',
            borderRadius: '50%',
            display: 'grid',
            placeItems: 'center',
            background: `color-mix(in srgb, ${checkColors[st]} 16%, transparent)`,
          }}
        >
          <svg width={8} height={8} viewBox="0 0 24 24" fill="none" stroke={checkColors[st]} strokeWidth={3.4} strokeLinecap="round" strokeLinejoin="round">
            <path d={st === 'pass' ? 'M4 12.5 9.5 18 20 6.5' : st === 'warn' ? 'M12 5v9M12 18.5v.01' : 'M5 5l14 14M19 5 5 19'} />
          </svg>
        </span>
        <span style={{ fontSize: 12.5, color: 'var(--muted)', fontWeight: 500, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {i.label ?? 'check'}
        </span>
        <span style={{ fontSize: 10, color: checkColors[st], marginLeft: 'auto', fontWeight: 700, letterSpacing: '.5px' }}>
          {st.toUpperCase()}
        </span>
      </div>
    )
  }

  return (
    <div>
      <div
        style={{ display: 'flex', alignItems: 'center', gap: 9, cursor: 'pointer' }}
        onClick={() => setOpen(!open)}
        title="Show tool input/output"
      >
        {pending && (
          <span
            style={{
              width: 13,
              height: 13,
              flex: '0 0 13px',
              borderRadius: '50%',
              border: '2px solid var(--accentSoft)',
              borderTopColor: 'var(--accent)',
              animation: 'spinDot .7s linear infinite',
            }}
          />
        )}
        {!pending && !tool.isError && (
          <svg width={13} height={13} viewBox="0 0 24 24" fill="none" stroke="var(--green)" strokeWidth={2.6} strokeLinecap="round" strokeLinejoin="round" style={{ flex: '0 0 13px' }}>
            <path d="M4 12.5 9.5 18 20 6.5" />
          </svg>
        )}
        {!pending && tool.isError && (
          <svg width={13} height={13} viewBox="0 0 24 24" fill="none" stroke="var(--red)" strokeWidth={2.6} strokeLinecap="round" strokeLinejoin="round" style={{ flex: '0 0 13px' }}>
            <path d="M5 5l14 14M19 5 5 19" />
          </svg>
        )}
        <span
          style={{
            fontSize: 12.5,
            color: pending ? 'var(--text)' : tool.isError ? 'var(--red)' : 'var(--muted)',
            fontWeight: 500,
            minWidth: 0,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {toolLabel(tool.name, tool.input)}
        </span>
        <span style={{ fontSize: 11, color: tool.isError ? 'var(--red)' : 'var(--muted)', marginLeft: 'auto', fontFamily: mono, whiteSpace: 'nowrap' }}>
          {toolMeta(tool)}
        </span>
      </div>
      {open && (
        <div
          style={{
            marginTop: 6,
            marginLeft: 22,
            fontFamily: mono,
            fontSize: 10,
            lineHeight: 1.5,
            background: '#0a0c0f',
            border: '1px solid var(--border)',
            borderRadius: 8,
            padding: '8px 10px',
            userSelect: 'text',
          }}
        >
          <div style={{ color: 'var(--muted)' }}>input · {tool.name}</div>
          <pre style={{ margin: '2px 0 6px', whiteSpace: 'pre-wrap', overflowX: 'auto', color: '#cdd6e4' }}>
            {JSON.stringify(tool.input, null, 1)}
          </pre>
          {tool.output !== undefined && (
            <>
              <div style={{ color: 'var(--muted)' }}>output</div>
              <pre
                style={{
                  margin: '2px 0 0',
                  whiteSpace: 'pre-wrap',
                  maxHeight: 176,
                  overflow: 'auto',
                  color: tool.isError ? 'var(--red)' : '#cdd6e4',
                }}
              >
                {tool.output}
              </pre>
            </>
          )}
        </div>
      )}
    </div>
  )
}

const emptySuggestions = [
  { label: '⚡ Build a 3-router OSPF triangle with FRR and verify adjacencies', text: 'Build a 3-router OSPF triangle with FRR and verify adjacencies' },
  { label: '🌐 eBGP peering lab: AS 65001 ↔ AS 65100 ↔ AS 65002', text: 'Create an eBGP peering lab: AS 65001 ↔ AS 65100 ↔ AS 65002 with FRR routers, then verify the BGP sessions' },
  { label: '🕸️ VXLAN EVPN spine-leaf fabric with two hosts', text: 'Build a VXLAN EVPN spine-leaf fabric with two hosts and test the L2 stretch' },
]

const followupSuggestions = [
  { label: '⚠️ Fail a link and watch reconvergence', text: 'Pick a link that has a redundant path, suspend it, verify the routing protocol reconverges onto the backup path, then report validation checks.' },
  { label: '✓ Re-run validation', text: 'Re-run validation on the current lab: check protocol adjacencies, routes, and end-to-end reachability with run_command, and report each result with report_check.' },
]

export default function AgentChat() {
  const items = useStore((s) => s.agentItems)
  const busy = useStore((s) => s.agentBusy)
  const connected = useStore((s) => s.agentConnected)
  const sendAgent = useStore((s) => s.sendAgent)
  const connectAgent = useStore((s) => s.connectAgent)
  const setInspectorTab = useStore((s) => s.setInspectorTab)
  const system = useStore((s) => s.system)
  const lab = useStore((s) => s.lab)
  const [input, setInput] = useState('')
  const chatRef = useRef<HTMLDivElement>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const blocks = useMemo(() => groupItems(items), [items])
  const deployed = Object.keys(lab?.nodes ?? {}).length > 0
  const hasConversation = items.some((i) => i.kind !== 'error')
  const ai = system?.ai

  useEffect(() => {
    const el = chatRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [items, busy])

  const send = (text: string, files?: string[]) => {
    if (sendAgent(text, files)) setInput('')
  }

  const importConfigs = async (list: FileList) => {
    const files = [...list].slice(0, 8)
    const bodies = await Promise.all(
      files.map(async (f) => `--- ${f.name} ---\n${(await f.text()).slice(0, 20000)}`),
    )
    const sent = sendAgent(
      `Build a digital twin of these production device configs: recreate the topology they imply (infer links from interface subnets), apply each config to the matching node, and validate the twin against what the configs promise.\n\n${bodies.join('\n\n')}`,
      files.map((f) => f.name),
    )
    // The file dialog can outlive the connection or an incoming turn.
    if (!sent) alert('Import not sent — the agent is busy or disconnected. Try again in a moment.')
  }

  const chips = busy ? [] : hasConversation ? followupSuggestions : emptySuggestions

  return (
    <div
      style={{
        width: 390,
        flex: '0 1 390px',
        minWidth: 270,
        background: 'var(--panel)',
        borderRight: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
        minHeight: 0,
      }}
    >
      <div
        ref={chatRef}
        style={{ flex: 1, overflowY: 'auto', padding: '18px 16px 8px', display: 'flex', flexDirection: 'column', gap: 16 }}
      >
        {!hasConversation && (
          <div style={{ margin: 'auto 8px', textAlign: 'center', padding: '40px 10px' }}>
            <div style={{ width: 52, height: 52, borderRadius: 16, background: 'var(--accentSoft)', display: 'grid', placeItems: 'center', margin: '0 auto 14px' }}>
              {stratoMark({ size: 26, inner: 'var(--accent)' })}
            </div>
            <div style={{ fontFamily: grotesk, fontSize: 17, fontWeight: 600, letterSpacing: '-.3px' }}>
              Describe a network
            </div>
            <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.6, marginTop: 6 }}>
              I design the topology, write per-vendor configs, boot real nodes, and verify protocols
              on their consoles — every action audited below as it runs.
            </div>
            {ai && !ai.available && (
              <div style={{ fontSize: 11.5, color: 'var(--amber)', marginTop: 12, lineHeight: 1.5 }}>
                Agent offline — set OPENROUTER_API_KEY or ANTHROPIC_API_KEY on the server.
              </div>
            )}
          </div>
        )}

        {blocks.map((b, bi) => (
          <div key={bi} style={{ display: 'flex', flexDirection: 'column', animation: 'fadeUp .25s ease' }}>
            {b.kind === 'user' && (
              <div
                style={{
                  alignSelf: 'flex-end',
                  maxWidth: '88%',
                  background: 'var(--accentSoft)',
                  border: '1px solid rgba(56,209,186,.25)',
                  borderRadius: '14px 14px 4px 14px',
                  padding: '10px 14px',
                  fontSize: 13,
                  lineHeight: 1.55,
                  userSelect: 'text',
                  whiteSpace: 'pre-wrap',
                  overflowWrap: 'anywhere',
                }}
              >
                {b.files?.length ? b.text.split('\n\n---')[0] : b.text}
                {!!b.files?.length && (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
                    {b.files.map((f, fi) => (
                      <span
                        key={fi}
                        style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, fontFamily: mono, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 6, padding: '4px 8px' }}
                      >
                        <svg width={11} height={11} viewBox="0 0 24 24" fill="none" stroke="var(--muted)" strokeWidth={2}>
                          <path d="M13 3H6v18h12V8z" />
                          <path d="M13 3v5h5" />
                        </svg>
                        {f}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            )}
            {b.kind === 'run' && (
              <div style={{ display: 'flex', gap: 10, maxWidth: '100%' }}>
                <div style={{ width: 26, height: 26, flex: '0 0 26px', borderRadius: 8, background: 'linear-gradient(135deg,var(--accent),#2a8fd1)', display: 'grid', placeItems: 'center', marginTop: 2 }}>
                  {stratoMark({ size: 13 })}
                </div>
                <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 8 }}>
                  {b.parts.map((p, pi) => {
                    if (p.kind === 'text') {
                      return (
                        <div key={pi} style={{ fontSize: 13, lineHeight: 1.6, userSelect: 'text', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>
                          {p.text}
                        </div>
                      )
                    }
                    if (p.kind === 'steps') {
                      return (
                        <div key={pi} style={{ display: 'flex', flexDirection: 'column', gap: 7, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 11, padding: '11px 13px' }}>
                          {p.tools.map((t) => (
                            <StepRow key={t.id} tool={t} />
                          ))}
                        </div>
                      )
                    }
                    if (p.kind === 'report') {
                      const bad = p.fail > 0
                      const warnOnly = !bad && p.warn > 0
                      const fg = bad ? 'var(--red)' : warnOnly ? 'var(--amber)' : 'var(--green)'
                      return (
                        <div
                          key={pi}
                          onClick={() => setInspectorTab('validation')}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: 10,
                            background: bad ? 'rgba(240,101,90,.07)' : warnOnly ? 'rgba(232,179,72,.08)' : 'rgba(62,207,142,.07)',
                            border: `1px solid ${bad ? 'rgba(240,101,90,.3)' : warnOnly ? 'rgba(232,179,72,.3)' : 'rgba(62,207,142,.28)'}`,
                            borderRadius: 11,
                            padding: '11px 13px',
                            cursor: 'pointer',
                          }}
                        >
                          <svg width={17} height={17} viewBox="0 0 24 24" fill="none" stroke={fg} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
                            <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" />
                          </svg>
                          <div style={{ flex: 1 }}>
                            <div style={{ fontSize: 12.5, fontWeight: 600, color: fg }}>
                              {p.passed === p.total
                                ? `All ${p.total} validation checks passed`
                                : `${p.passed}/${p.total} checks passed${p.warn ? ` · ${p.warn} warning${p.warn > 1 ? 's' : ''}` : ''}${p.fail ? ` · ${p.fail} failed` : ''}`}
                            </div>
                            <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 1 }}>Validated on real device consoles</div>
                          </div>
                          <span style={{ fontSize: 11, color: 'var(--muted)' }}>View →</span>
                        </div>
                      )
                    }
                    return (
                      <div
                        key={pi}
                        style={{ fontSize: 12, lineHeight: 1.5, color: 'var(--red)', background: 'rgba(240,101,90,.07)', border: '1px solid rgba(240,101,90,.3)', borderRadius: 10, padding: '9px 12px', userSelect: 'text', overflowWrap: 'anywhere' }}
                      >
                        {p.text}
                      </div>
                    )
                  })}
                </div>
              </div>
            )}
          </div>
        ))}

        {busy && !items.some((i) => i.kind === 'tool' && i.output === undefined) && (
          <div style={{ display: 'flex', gap: 10, alignItems: 'center', marginLeft: 36 }}>
            <span style={{ width: 13, height: 13, borderRadius: '50%', border: '2px solid var(--accentSoft)', borderTopColor: 'var(--accent)', animation: 'spinDot .7s linear infinite' }} />
            <span style={{ fontSize: 12.5, color: 'var(--muted)' }}>thinking…</span>
          </div>
        )}
      </div>

      {/* suggestions + composer */}
      <div style={{ padding: '10px 14px 14px', borderTop: '1px solid var(--border)' }}>
        {chips.length > 0 && (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 10 }}>
            {chips.map((c, ci) => (
              <button
                key={ci}
                className="chip"
                onClick={() => sendAgent(c.text)}
                disabled={busy || !connected}
                style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--muted)', borderRadius: 999, padding: '5px 11px', fontSize: 11.5, cursor: 'pointer', textAlign: 'left', maxWidth: '100%', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}
              >
                {c.label}
              </button>
            ))}
          </div>
        )}
        {!connected && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8, fontSize: 11.5, color: 'var(--muted)' }}>
            <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--red)' }} />
            {ai && !ai.available ? 'Agent needs an API key on the server' : 'Agent disconnected'}
            {lab && (
              <button
                className="btn-ghost"
                onClick={() => connectAgent(lab.id, false)}
                style={{ marginLeft: 'auto', border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 7, padding: '3px 9px', fontSize: 10.5, fontWeight: 600, cursor: 'pointer' }}
              >
                Reconnect
              </button>
            )}
          </div>
        )}
        <div
          className="composer"
          style={{ display: 'flex', alignItems: 'flex-end', gap: 8, background: 'var(--panel2)', border: '1px solid var(--border2)', borderRadius: 13, padding: '9px 10px' }}
        >
          <input
            ref={fileRef}
            type="file"
            multiple
            accept=".cfg,.conf,.txt,.json,.frr,.ios"
            className="hidden"
            onChange={(e) => {
              if (e.target.files?.length) void importConfigs(e.target.files)
              e.target.value = ''
            }}
          />
          <button
            title="Import device configs → digital twin"
            className="iconbtn"
            onClick={() => fileRef.current?.click()}
            disabled={busy || !connected}
            style={{ border: 'none', background: 'transparent', color: 'var(--muted)', cursor: 'pointer', width: 28, height: 28, borderRadius: 8, display: 'grid', placeItems: 'center', flex: '0 0 28px' }}
          >
            <svg width={16} height={16} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 12.5 12.6 21a5.4 5.4 0 0 1-7.6-7.6L14 4.3a3.6 3.6 0 0 1 5.1 5.1L10.7 18a1.8 1.8 0 0 1-2.5-2.5l7.8-7.9" />
            </svg>
          </button>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault()
                send(input)
              }
            }}
            placeholder={
              !connected
                ? 'Agent unavailable…'
                : busy
                  ? 'Agent is working…'
                  : deployed
                    ? 'Describe a change — the agent updates every device…'
                    : 'Describe the network you need…'
            }
            rows={1}
            disabled={!connected}
            style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', resize: 'none', color: 'var(--text)', fontSize: 13, lineHeight: 1.5, maxHeight: 96, padding: '4px 0', fontFamily: 'inherit' }}
          />
          <button
            onClick={() => send(input)}
            disabled={busy || !connected || !input.trim()}
            style={{ border: 'none', background: busy || !connected || !input.trim() ? 'var(--border2)' : 'var(--accent)', color: '#08211d', width: 30, height: 30, borderRadius: 9, cursor: 'pointer', display: 'grid', placeItems: 'center', flex: '0 0 30px' }}
          >
            <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 19V5M5 12l7-7 7 7" />
            </svg>
          </button>
        </div>
        {ai?.available && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginTop: 7, fontSize: 10, color: 'var(--muted)', fontFamily: mono, justifyContent: 'flex-end' }}>
            <span style={{ width: 5, height: 5, borderRadius: '50%', background: connected ? 'var(--green)' : 'var(--red)' }} />
            {ai.model.split('/').pop()}
          </div>
        )}
      </div>
    </div>
  )
}
