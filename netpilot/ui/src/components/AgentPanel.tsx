// Agentic chat: streaming AgentEvents rendered as an auditable run
// timeline — tool cards with per-tool icons and live status, grouped
// working steps, provider/model chip.

import { useEffect, useRef, useState } from 'react'
import {
  Activity,
  Bot,
  Cable,
  ChevronDown,
  ChevronRight,
  Eye,
  FileText,
  Gauge,
  LayoutGrid,
  Play,
  Plus,
  Send,
  Sparkles,
  Square,
  Terminal,
  Trash2,
  X,
  type LucideIcon,
} from 'lucide-react'
import { api, wsUrl, type SystemStatus } from '../api'
import { useStore } from '../store'

type ChatItem =
  | { kind: 'user'; text: string }
  | { kind: 'assistant'; text: string }
  | { kind: 'tool'; id: string; name: string; input: unknown; output?: string; isError?: boolean }
  | { kind: 'error'; text: string }

const toolMeta: Record<string, { icon: LucideIcon; label: (input: never) => string }> = {
  get_lab: { icon: Eye, label: () => 'Reading the lab topology' },
  list_templates: { icon: LayoutGrid, label: () => 'Checking available devices' },
  create_node: {
    icon: Plus,
    label: (i: { name?: string; template?: string }) =>
      `Adding ${i.name ?? 'node'} (${i.template ?? '?'})`,
  },
  update_node: { icon: FileText, label: (i: { name?: string }) => `Updating ${i.name ?? 'node'}` },
  delete_node: { icon: Trash2, label: (i: { name?: string }) => `Removing ${i.name ?? 'node'}` },
  create_link: {
    icon: Cable,
    label: (i: { a_node?: string; b_node?: string; network?: string }) =>
      `Cabling ${i.a_node ?? '?'} ↔ ${i.b_node ?? i.network ?? '?'}`,
  },
  create_network: {
    icon: Cable,
    label: (i: { name?: string }) => `Creating network ${i.name ?? ''}`,
  },
  set_startup_config: {
    icon: FileText,
    label: (i: { node?: string }) => `Writing config for ${i.node ?? 'node'}`,
  },
  start: { icon: Play, label: (i: { node?: string }) => `Starting ${i.node ?? 'the lab'}` },
  stop: { icon: Square, label: (i: { node?: string }) => `Stopping ${i.node ?? 'the lab'}` },
  run_command: {
    icon: Terminal,
    label: (i: { node?: string; command?: string }) =>
      `${i.node ?? '?'} $ ${(i.command ?? '').slice(0, 48)}`,
  },
  set_link_quality: {
    icon: Gauge,
    label: (i: { a_node?: string; b_node?: string }) =>
      `Tuning link ${i.a_node ?? '?'} ↔ ${i.b_node ?? '?'}`,
  },
}

const suggestions = [
  'Build a 3-router OSPF triangle with FRR and verify adjacencies',
  'Create an eBGP peering lab: AS 65001 ↔ AS 65100 ↔ AS 65002',
  'Build a VXLAN EVPN spine-leaf fabric with two hosts and test L2 stretch',
  'Why is r1 not forming an OSPF adjacency? Investigate and fix it',
]

export default function AgentPanel() {
  const lab = useStore((s) => s.lab)
  const refreshLab = useStore((s) => s.refreshLab)
  const toggleAgent = useStore((s) => s.toggleAgent)
  const [items, setItems] = useState<ChatItem[]>([])
  const [input, setInput] = useState('')
  const [busy, setBusy] = useState(false)
  const [connected, setConnected] = useState(false)
  const [ai, setAi] = useState<SystemStatus['ai'] | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    api.system().then((s) => setAi(s.ai)).catch(() => {})
  }, [])

  useEffect(() => {
    if (!lab) return
    const ws = new WebSocket(wsUrl(`/api/ws/agent/${lab.id}`))
    ws.onopen = () => setConnected(true)
    ws.onclose = () => {
      setConnected(false)
      setBusy(false)
    }
    ws.onmessage = (msg) => {
      try {
        const ev = JSON.parse(msg.data)
        setItems((prev) => {
          const next = [...prev]
          if (ev.type === 'text') {
            next.push({ kind: 'assistant', text: ev.text })
          } else if (ev.type === 'tool_call') {
            next.push({ kind: 'tool', id: ev.id, name: ev.name, input: ev.input })
          } else if (ev.type === 'tool_result') {
            const idx = next.findIndex((i) => i.kind === 'tool' && i.id === ev.id)
            if (idx >= 0) {
              const t = next[idx] as Extract<ChatItem, { kind: 'tool' }>
              next[idx] = { ...t, output: ev.output, isError: ev.is_error }
            }
            void refreshLab()
          } else if (ev.type === 'error') {
            next.push({ kind: 'error', text: ev.message })
          } else if (ev.type === 'done') {
            void refreshLab()
          }
          return next
        })
        if (ev.type === 'done' || ev.type === 'error') setBusy(false)
      } catch {
        /* ignore */
      }
    }
    wsRef.current = ws
    return () => {
      ws.close()
      wsRef.current = null
    }
  }, [lab?.id]) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [items, busy])

  const send = (text?: string) => {
    const message = (text ?? input).trim()
    if (!message || busy || !wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return
    setItems((prev) => [...prev, { kind: 'user', text: message }])
    wsRef.current.send(JSON.stringify({ message }))
    setInput('')
    setBusy(true)
  }

  const pendingTool = items.some((i) => i.kind === 'tool' && i.output === undefined)

  return (
    <aside className="flex w-[26rem] shrink-0 flex-col border-l border-ink-800 bg-ink-900">
      <div className="flex items-center gap-2 border-b border-ink-800 px-3 py-2">
        <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-accent-600/20 text-accent-500">
          <Sparkles size={15} />
        </div>
        <div>
          <h3 className="text-sm font-medium leading-tight text-white">Lab Agent</h3>
          <p className="text-[10px] leading-tight text-ink-500">
            {ai?.available ? (
              <>
                <span className="text-emerald-500">●</span> {ai.model}
              </>
            ) : (
              <>
                <span className="text-red-400">●</span> no API key on server
              </>
            )}
          </p>
        </div>
        <button
          onClick={toggleAgent}
          className="ml-auto rounded p-1 text-ink-400 hover:bg-ink-700 hover:text-white"
        >
          <X size={14} />
        </button>
      </div>

      <div className="min-h-0 flex-1 space-y-2.5 overflow-y-auto p-3">
        {items.length === 0 && (
          <div className="mt-6 text-center">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-ink-850">
              <Bot size={24} className="text-accent-500" />
            </div>
            <p className="mt-3 text-sm font-medium text-ink-200">
              Describe the network you want.
            </p>
            <p className="mx-auto mt-1 max-w-72 text-xs leading-relaxed text-ink-500">
              The agent designs the topology, writes vendor configs, boots nodes, and verifies
              protocols on the real consoles — every action shown below as it runs.
            </p>
            <div className="mx-auto mt-5 max-w-80 space-y-1.5 text-left">
              {suggestions.map((s) => (
                <button
                  key={s}
                  onClick={() => send(s)}
                  disabled={!connected}
                  className="block w-full rounded-xl border border-ink-800 bg-ink-950/60 px-3 py-2 text-left text-xs leading-relaxed text-ink-300 transition hover:border-accent-600/50 hover:text-ink-100 disabled:opacity-50"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}
        {items.map((item, i) => (
          <ChatBubble key={i} item={item} />
        ))}
        {busy && !pendingTool && (
          <div className="mr-8 flex items-center gap-2.5 rounded-xl border border-ink-800 bg-ink-950/60 px-3 py-2.5">
            <Activity size={14} className="animate-pulse text-accent-500" />
            <span className="bg-gradient-to-r from-ink-400 via-white to-ink-400 bg-[length:200%_100%] bg-clip-text text-xs text-transparent [animation:shimmer_2s_linear_infinite]">
              thinking…
            </span>
          </div>
        )}
        <div ref={endRef} />
      </div>

      <div className="border-t border-ink-800 p-2">
        <div className="flex items-end gap-2 rounded-xl border border-ink-700 bg-ink-950 p-2 focus-within:border-accent-600">
          <textarea
            rows={2}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                send()
              }
            }}
            placeholder={
              connected
                ? 'Build, configure, break, or debug this lab…'
                : 'Agent unavailable — set OPENROUTER_API_KEY or ANTHROPIC_API_KEY on the server'
            }
            disabled={!connected}
            className="max-h-32 w-full resize-none bg-transparent text-sm text-white outline-none placeholder:text-ink-600"
          />
          <button
            onClick={() => send()}
            disabled={busy || !input.trim() || !connected}
            className="rounded-lg bg-accent-600 p-2 text-white transition hover:bg-accent-500 disabled:opacity-40"
          >
            <Send size={14} />
          </button>
        </div>
      </div>
    </aside>
  )
}

function ChatBubble({ item }: { item: ChatItem }) {
  const [open, setOpen] = useState(false)
  if (item.kind === 'user') {
    return (
      <div className="ml-10 rounded-2xl rounded-br-md bg-accent-600/20 px-3.5 py-2 text-sm text-ink-100">
        {item.text}
      </div>
    )
  }
  if (item.kind === 'assistant') {
    return (
      <div className="mr-6 whitespace-pre-wrap rounded-2xl rounded-bl-md bg-ink-850 px-3.5 py-2.5 text-sm leading-relaxed text-ink-200">
        {item.text}
      </div>
    )
  }
  if (item.kind === 'error') {
    return (
      <div className="rounded-xl border border-red-900 bg-red-950/40 px-3 py-2 text-xs text-red-300">
        {item.text}
      </div>
    )
  }

  const meta = toolMeta[item.name]
  const Icon = meta?.icon ?? Terminal
  const label = meta ? meta.label(item.input as never) : item.name
  const pending = item.output === undefined
  return (
    <div className="mr-6 overflow-hidden rounded-xl border border-ink-800 bg-ink-950/70">
      <button
        className="flex w-full items-center gap-2.5 px-3 py-2 text-left hover:bg-ink-900"
        onClick={() => setOpen(!open)}
      >
        <span
          className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-md ${
            pending
              ? 'bg-accent-600/20 text-accent-500'
              : item.isError
                ? 'bg-red-900/40 text-red-400'
                : 'bg-emerald-900/30 text-emerald-500'
          }`}
        >
          <Icon size={13} />
        </span>
        <span className="min-w-0 flex-1 truncate text-xs text-ink-200">{label}</span>
        {pending ? (
          <span className="h-3 w-3 shrink-0 animate-spin rounded-full border border-ink-600 border-t-accent-500" />
        ) : (
          <span
            className={`shrink-0 text-[10px] font-medium ${item.isError ? 'text-red-400' : 'text-emerald-500'}`}
          >
            {item.isError ? 'failed' : 'done'}
          </span>
        )}
        {open ? (
          <ChevronDown size={12} className="shrink-0 text-ink-500" />
        ) : (
          <ChevronRight size={12} className="shrink-0 text-ink-500" />
        )}
      </button>
      {open && (
        <div className="border-t border-ink-800 p-2.5 font-mono text-[10px] leading-relaxed">
          <div className="text-ink-500">input · {item.name}</div>
          <pre className="mb-1.5 overflow-x-auto whitespace-pre-wrap text-ink-300">
            {JSON.stringify(item.input, null, 1)}
          </pre>
          {item.output !== undefined && (
            <>
              <div className="text-ink-500">output</div>
              <pre
                className={`max-h-44 overflow-auto whitespace-pre-wrap ${
                  item.isError ? 'text-red-300' : 'text-ink-300'
                }`}
              >
                {item.output}
              </pre>
            </>
          )}
        </div>
      )}
    </div>
  )
}
