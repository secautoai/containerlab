// Bottom console panel: xterm.js tabs bridged to node serial consoles
// over WebSocket, plus an activity log tab.

import { useEffect, useRef } from 'react'
import { ChevronDown, Terminal as TerminalIcon, X } from 'lucide-react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { wsUrl } from '../api'
import { useStore } from '../store'

function XTerm({ labId, nodeId }: { labId: string; nodeId: string }) {
  const holder = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = holder.current
    if (!el) return
    const term = new Terminal({
      fontFamily: "'JetBrains Mono', ui-monospace, monospace",
      fontSize: 13,
      cursorBlink: true,
      theme: {
        background: '#0a0e14',
        foreground: '#cbd5e1',
        cursor: '#22d3ee',
        selectionBackground: '#33415c',
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

  return <div ref={holder} className="h-full w-full" />
}

function LogView() {
  const logs = useStore((s) => s.logs)
  const end = useRef<HTMLDivElement>(null)
  useEffect(() => {
    end.current?.scrollIntoView({ behavior: 'smooth' })
  }, [logs])
  return (
    <div className="h-full overflow-y-auto p-2 font-mono text-xs">
      {logs.length === 0 && <p className="text-ink-600">No activity yet.</p>}
      {logs.map((l, i) => (
        <div key={i} className="flex gap-2">
          <span className="shrink-0 text-ink-600">
            {new Date(l.at).toLocaleTimeString()}
          </span>
          <span
            className={
              l.level === 'error' ? 'text-red-400' : l.level === 'warn' ? 'text-amber-400' : 'text-ink-300'
            }
          >
            {l.message}
          </span>
        </div>
      ))}
      <div ref={end} />
    </div>
  )
}

export default function ConsolePanel() {
  const lab = useStore((s) => s.lab)
  const consoles = useStore((s) => s.consoles)
  const active = useStore((s) => s.activeConsole)
  const setActive = useStore((s) => s.setActiveConsole)
  const closeConsole = useStore((s) => s.closeConsole)
  const setConsoleOpen = useStore((s) => s.setConsoleOpen)

  if (!lab) return null

  return (
    <div className="flex h-72 shrink-0 flex-col border-t border-ink-800 bg-ink-950">
      <div className="flex items-center border-b border-ink-800 bg-ink-900">
        <button
          onClick={() => setActive('__log__')}
          className={`px-3 py-1.5 text-xs ${
            active === '__log__' ? 'bg-ink-950 text-white' : 'text-ink-400 hover:text-white'
          }`}
        >
          Activity
        </button>
        {consoles.map((c) => (
          <div
            key={c.nodeId}
            className={`group flex cursor-pointer items-center gap-1.5 px-3 py-1.5 text-xs ${
              active === c.nodeId ? 'bg-ink-950 text-white' : 'text-ink-400 hover:text-white'
            }`}
            onClick={() => setActive(c.nodeId)}
          >
            <TerminalIcon size={12} />
            <span className="font-mono">{c.nodeName}</span>
            <button
              className="rounded p-0.5 opacity-0 hover:bg-ink-700 group-hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation()
                closeConsole(c.nodeId)
              }}
            >
              <X size={11} />
            </button>
          </div>
        ))}
        <button
          onClick={() => setConsoleOpen(false)}
          className="ml-auto p-1.5 text-ink-400 hover:text-white"
          title="Collapse"
        >
          <ChevronDown size={15} />
        </button>
      </div>
      <div className="min-h-0 flex-1">
        {active === '__log__' || active === null ? (
          <LogView />
        ) : (
          consoles.map((c) => (
            <div key={c.nodeId} className="h-full" style={{ display: active === c.nodeId ? 'block' : 'none' }}>
              <XTerm labId={lab.id} nodeId={c.nodeId} />
            </div>
          ))
        )}
      </div>
    </div>
  )
}
