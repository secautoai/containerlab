// Lab editor: toolbar + palette + canvas + side panel + console + agent.

import { useState } from 'react'
import {
  ArrowLeft,
  Download,
  Network as NetworkIcon,
  Play,
  Sparkles,
  Square as SquareIcon,
  SquareDashed,
  Terminal,
  Type,
} from 'lucide-react'
import { api, type NetworkKind } from '../api'
import { useStore } from '../store'
import Canvas, { type Selection } from './Canvas'
import Palette from './Palette'
import SidePanel from './SidePanel'
import ConsolePanel from './ConsolePanel'
import AgentPanel from './AgentPanel'
import { stateColor } from './icons'

export default function LabEditor() {
  const lab = useStore((s) => s.lab)
  const states = useStore((s) => s.states)
  const openDashboard = useStore((s) => s.openDashboard)
  const refreshLab = useStore((s) => s.refreshLab)
  const consoleOpen = useStore((s) => s.consoleOpen)
  const setConsoleOpen = useStore((s) => s.setConsoleOpen)
  const agentOpen = useStore((s) => s.agentOpen)
  const toggleAgent = useStore((s) => s.toggleAgent)
  const pushLog = useStore((s) => s.pushLog)
  const [selection, setSelection] = useState<Selection | null>(null)
  const [netMenu, setNetMenu] = useState(false)

  if (!lab) {
    return (
      <div className="flex h-full items-center justify-center text-ink-400">
        <span className="h-5 w-5 animate-spin rounded-full border-2 border-ink-700 border-t-accent-500" />
      </div>
    )
  }

  const nodeCount = Object.keys(lab.nodes).length
  const runningCount = Object.values(states).filter((s) => s === 'running').length
  const anyRunning = Object.values(states).some((s) => s === 'running' || s === 'starting')

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
        color: kind === 'text' ? '#94a3b8' : '#33415c',
        fill: kind === 'rect' ? 'rgba(34,211,238,0.05)' : '',
        font_size: 14,
        z: kind === 'rect' ? -1 : 1,
      })
      await refreshLab()
    } catch (e) {
      pushLog('error', `annotation: ${e instanceof Error ? e.message : e}`)
    }
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex shrink-0 items-center gap-2 border-b border-ink-800 bg-ink-900 px-3 py-2">
        <button
          onClick={openDashboard}
          className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-ink-300 hover:bg-ink-800 hover:text-white"
        >
          <ArrowLeft size={15} /> Labs
        </button>
        <div className="mx-2 h-5 w-px bg-ink-700" />
        <h2 className="font-medium text-white">{lab.name}</h2>
        <span className="text-xs text-ink-500">
          {nodeCount} nodes · {runningCount} running
          {!lab.kvm && (
            <span className="ml-2 rounded bg-amber-950 px-1.5 py-0.5 text-[10px] text-amber-400" title="No /dev/kvm — nodes run under slow TCG emulation">
              no KVM
            </span>
          )}
        </span>

        <div className="ml-auto flex items-center gap-1.5">
          {!anyRunning ? (
            <button
              onClick={async () => {
                pushLog('info', 'starting lab…')
                await api.startLab(lab.id)
              }}
              className="flex items-center gap-1.5 rounded-md bg-emerald-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-emerald-600"
            >
              <Play size={14} /> Start lab
            </button>
          ) : (
            <button
              onClick={async () => {
                await api.stopLab(lab.id)
                await refreshLab()
              }}
              className="flex items-center gap-1.5 rounded-md bg-red-800 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700"
            >
              <SquareIcon size={14} /> Stop lab
            </button>
          )}

          <div className="relative">
            <button
              onClick={() => setNetMenu(!netMenu)}
              title="Add network segment"
              className="flex items-center gap-1.5 rounded-md border border-ink-700 px-2.5 py-1.5 text-sm text-ink-300 hover:border-ink-600 hover:text-white"
            >
              <NetworkIcon size={14} /> Network
            </button>
            {netMenu && (
              <div className="absolute right-0 top-full z-50 mt-1 w-44 overflow-hidden rounded-lg border border-ink-700 bg-ink-850 py-1 shadow-xl">
                {(['bridge', 'nat', 'management', 'cloud'] as NetworkKind[]).map((k) => (
                  <button
                    key={k}
                    onClick={() => void addNetwork(k)}
                    className="block w-full px-3 py-1.5 text-left text-sm text-ink-200 hover:bg-ink-700"
                  >
                    {k}
                  </button>
                ))}
              </div>
            )}
          </div>

          <button
            onClick={() => void addAnnotation('text')}
            title="Add text label"
            className="rounded-md border border-ink-700 p-1.5 text-ink-300 hover:border-ink-600 hover:text-white"
          >
            <Type size={14} />
          </button>
          <button
            onClick={() => void addAnnotation('rect')}
            title="Add region"
            className="rounded-md border border-ink-700 p-1.5 text-ink-300 hover:border-ink-600 hover:text-white"
          >
            <SquareDashed size={14} />
          </button>
          <a
            href={api.exportUrl(lab.id)}
            title="Export lab"
            className="rounded-md border border-ink-700 p-1.5 text-ink-300 hover:border-ink-600 hover:text-white"
          >
            <Download size={14} />
          </a>
          <button
            onClick={() => setConsoleOpen(!consoleOpen)}
            title="Consoles"
            className={`rounded-md border p-1.5 ${
              consoleOpen
                ? 'border-accent-600 text-accent-500'
                : 'border-ink-700 text-ink-300 hover:border-ink-600 hover:text-white'
            }`}
          >
            <Terminal size={14} />
          </button>
          <button
            onClick={toggleAgent}
            title="AI agent"
            className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium ${
              agentOpen ? 'bg-accent-600 text-white' : 'bg-ink-700 text-ink-200 hover:bg-ink-600'
            }`}
          >
            <Sparkles size={14} /> Agent
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <Palette />
        <main className="flex min-w-0 flex-1 flex-col">
          <div className="min-h-0 flex-1">
            <Canvas onSelect={setSelection} />
          </div>
          {consoleOpen && <ConsolePanel />}
        </main>
        {selection && <SidePanel selection={selection} onClose={() => setSelection(null)} />}
        {agentOpen && <AgentPanel />}
      </div>

      <footer className="flex shrink-0 items-center gap-3 border-t border-ink-800 bg-ink-900 px-3 py-1 text-[10px] text-ink-600">
        <span>double-click a node for its console · right-click for actions · drag between nodes to cable</span>
        <span className="ml-auto flex items-center gap-2">
          {(['running', 'starting', 'stopped', 'error'] as const).map((s) => (
            <span key={s} className="flex items-center gap-1">
              <span className="h-1.5 w-1.5 rounded-full" style={{ background: stateColor[s] }} />
              {s}
            </span>
          ))}
        </span>
      </footer>
    </div>
  )
}
