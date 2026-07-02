// AI agent chat: streams AgentEvents over WebSocket with an auditable
// tool-call transcript.

import { useEffect, useRef, useState } from 'react'
import { Bot, ChevronDown, ChevronRight, Send, Sparkles, X } from 'lucide-react'
import { wsUrl } from '../api'
import { useStore } from '../store'

type ChatItem =
  | { kind: 'user'; text: string }
  | { kind: 'assistant'; text: string }
  | { kind: 'tool'; id: string; name: string; input: unknown; output?: string; isError?: boolean }
  | { kind: 'error'; text: string }

export default function AgentPanel() {
  const lab = useStore((s) => s.lab)
  const refreshLab = useStore((s) => s.refreshLab)
  const toggleAgent = useStore((s) => s.toggleAgent)
  const [items, setItems] = useState<ChatItem[]>([])
  const [input, setInput] = useState('')
  const [busy, setBusy] = useState(false)
  const [connected, setConnected] = useState(false)
  const wsRef = useRef<WebSocket | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

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
  }, [items])

  const send = () => {
    const text = input.trim()
    if (!text || busy || !wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return
    setItems((prev) => [...prev, { kind: 'user', text }])
    wsRef.current.send(JSON.stringify({ message: text }))
    setInput('')
    setBusy(true)
  }

  return (
    <aside className="flex w-96 shrink-0 flex-col border-l border-ink-800 bg-ink-900">
      <div className="flex items-center gap-2 border-b border-ink-800 px-3 py-2">
        <Sparkles size={15} className="text-accent-500" />
        <h3 className="text-sm font-medium text-white">Lab Agent</h3>
        <span
          className={`h-1.5 w-1.5 rounded-full ${connected ? 'bg-emerald-400' : 'bg-red-400'}`}
          title={connected ? 'connected' : 'disconnected'}
        />
        <button onClick={toggleAgent} className="ml-auto rounded p-1 text-ink-400 hover:bg-ink-700 hover:text-white">
          <X size={14} />
        </button>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
        {items.length === 0 && (
          <div className="mt-8 text-center text-xs leading-relaxed text-ink-500">
            <Bot size={28} className="mx-auto mb-2 text-ink-600" />
            Ask the agent to build, configure, or troubleshoot this lab.
            <div className="mx-auto mt-4 max-w-72 space-y-1.5 text-left">
              {[
                'Build a 3-router OSPF triangle with VyOS',
                'Add a management switch and connect every node',
                'Why is R1 not forming an adjacency with R2?',
                'Configure eBGP between R1 (AS 65001) and R2 (AS 65002)',
              ].map((s) => (
                <button
                  key={s}
                  onClick={() => setInput(s)}
                  className="block w-full rounded-lg border border-ink-800 px-2.5 py-1.5 text-left text-ink-400 hover:border-ink-600 hover:text-ink-200"
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
        {busy && (
          <div className="flex items-center gap-2 text-xs text-ink-500">
            <span className="h-3 w-3 animate-spin rounded-full border border-ink-600 border-t-accent-500" />
            working…
          </div>
        )}
        <div ref={endRef} />
      </div>

      <div className="border-t border-ink-800 p-2">
        <div className="flex items-end gap-2 rounded-lg border border-ink-700 bg-ink-950 p-1.5 focus-within:border-accent-600">
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
            placeholder={connected ? 'Tell the agent what to do…' : 'Agent unavailable (set ANTHROPIC_API_KEY on the server)'}
            disabled={!connected}
            className="max-h-32 w-full resize-none bg-transparent text-sm text-white outline-none placeholder:text-ink-600"
          />
          <button
            onClick={send}
            disabled={busy || !input.trim() || !connected}
            className="rounded-md bg-accent-600 p-1.5 text-white hover:bg-accent-500 disabled:opacity-40"
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
      <div className="ml-8 rounded-xl rounded-br-sm bg-accent-600/20 px-3 py-2 text-sm text-ink-200">
        {item.text}
      </div>
    )
  }
  if (item.kind === 'assistant') {
    return (
      <div className="mr-4 whitespace-pre-wrap rounded-xl rounded-bl-sm bg-ink-850 px-3 py-2 text-sm leading-relaxed text-ink-200">
        {item.text}
      </div>
    )
  }
  if (item.kind === 'error') {
    return (
      <div className="rounded-lg border border-red-900 bg-red-950/40 px-3 py-2 text-xs text-red-300">
        {item.text}
      </div>
    )
  }
  // tool call
  const pending = item.output === undefined
  return (
    <div className="mr-4 overflow-hidden rounded-lg border border-ink-800 bg-ink-950 text-xs">
      <button
        className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left hover:bg-ink-900"
        onClick={() => setOpen(!open)}
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <span className="font-mono text-accent-500">{item.name}</span>
        {pending ? (
          <span className="ml-auto h-2.5 w-2.5 animate-spin rounded-full border border-ink-600 border-t-accent-500" />
        ) : item.isError ? (
          <span className="ml-auto text-red-400">failed</span>
        ) : (
          <span className="ml-auto text-emerald-500">ok</span>
        )}
      </button>
      {open && (
        <div className="border-t border-ink-800 p-2 font-mono text-[10px] leading-relaxed">
          <div className="text-ink-500">input</div>
          <pre className="mb-1.5 overflow-x-auto whitespace-pre-wrap text-ink-300">
            {JSON.stringify(item.input, null, 1)}
          </pre>
          {item.output !== undefined && (
            <>
              <div className="text-ink-500">output</div>
              <pre
                className={`max-h-40 overflow-auto whitespace-pre-wrap ${
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
