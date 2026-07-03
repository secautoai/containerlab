// Zustand app store: current view, lab document, runtime states, consoles,
// the agent chat stream, validation checks, and the session audit log.

import { create } from 'zustand'
import {
  api,
  wsUrl,
  type LabView,
  type NodeState,
  type SystemStatus,
  type Template,
} from './api'

export interface ConsoleTab {
  nodeId: string
  nodeName: string
}

// One entry in the Sessions audit log (Strato: PROMPT/AGENT/STEP/DEPLOY/…).
export type SessionKind =
  | 'prompt'
  | 'agent'
  | 'step'
  | 'deploy'
  | 'check'
  | 'ssh'
  | 'cli'
  | 'error'
  | 'warn'
  | 'sys'

export interface SessionEntry {
  ts: number
  kind: SessionKind
  text: string
}

export type CheckStatus = 'pass' | 'warn' | 'fail'

// Models don't reliably respect the lowercase enum; never let an unknown
// status render green.
export function normalizeCheckStatus(s: string | undefined): CheckStatus {
  const v = (s ?? '').toLowerCase()
  return v === 'pass' || v === 'warn' || v === 'fail' ? v : 'warn'
}

export interface Check {
  label: string
  status: CheckStatus
  detail: string
  at: number
}

export type ChatItem =
  | { kind: 'user'; text: string; files?: string[] }
  | { kind: 'assistant'; text: string }
  | { kind: 'tool'; id: string; name: string; input: unknown; output?: string; isError?: boolean }
  | { kind: 'report'; passed: number; warn: number; fail: number; total: number }
  | { kind: 'error'; text: string }

export type InspectorTab = 'console' | 'configs' | 'validation' | 'sessions' | 'details'

interface AgentToolInput {
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

interface AppStore {
  view: { kind: 'dashboard' } | { kind: 'lab'; labId: string }
  templates: Template[]
  system: SystemStatus | null
  lab: LabView | null
  states: Record<string, NodeState>
  consoles: ConsoleTab[]
  activeConsole: string | null
  eventsSocket: WebSocket | null

  // agent chat
  agentItems: ChatItem[]
  agentBusy: boolean
  agentConnected: boolean
  agentSocket: WebSocket | null

  // inspector state
  inspectorTab: InspectorTab
  inspectorNode: string | null // node selected in the Configs tab
  checks: Check[]
  lastValidated: number | null
  sessions: SessionEntry[]
  cfgTouched: Record<string, number> // node name -> ts the agent rewrote its config

  openDashboard(): void
  openLab(labId: string): Promise<void>
  refreshLab(): Promise<void>
  loadTemplates(): Promise<void>
  loadSystem(): Promise<void>
  connectEvents(): void

  openConsole(nodeId: string, nodeName: string): void
  closeConsole(nodeId: string): void
  setActiveConsole(nodeId: string): void

  connectAgent(labId: string, fresh?: boolean): void
  sendAgent(message: string, files?: string[]): boolean
  setInspectorTab(tab: InspectorTab): void
  setInspectorNode(nodeId: string | null): void

  record(kind: SessionKind, text: string): void
  clearSessions(): void
  pushLog(level: string, message: string): void
}

// Strato-voice one-liners for the audit log.
function toolSessionEntry(name: string, input: AgentToolInput): { kind: SessionKind; text: string } | null {
  switch (name) {
    case 'create_node':
      return { kind: 'deploy', text: `Deployed ${input.name ?? 'node'} (${input.template ?? '?'})` }
    case 'delete_node':
      return { kind: 'deploy', text: `Removed ${input.name ?? 'node'}` }
    case 'create_link':
      return {
        kind: 'deploy',
        text: `Wired ${input.a_node ?? '?'} ↔ ${input.b_node ?? input.network ?? '?'}`,
      }
    case 'create_network':
      return { kind: 'deploy', text: `Created network ${input.name ?? ''}` }
    case 'update_node':
      return { kind: 'step', text: `Updated ${input.name ?? 'node'}` }
    case 'set_startup_config':
      return { kind: 'step', text: `Wrote config for ${input.node ?? 'node'}` }
    case 'start':
      return { kind: 'deploy', text: `Starting ${input.node ?? 'all nodes'}` }
    case 'stop':
      return { kind: 'deploy', text: `Stopping ${input.node ?? 'all nodes'}` }
    case 'run_command':
      return { kind: 'cli', text: `${input.node ?? '?'} $ ${input.command ?? ''}` }
    case 'set_link_quality':
      return {
        kind: 'deploy',
        text: `${input.suspended ? 'Failed' : 'Tuned'} link ${input.a_node ?? '?'} ↔ ${input.b_node ?? '?'}`,
      }
    case 'report_check':
      return {
        kind: 'check',
        text: `${(input.status ?? 'pass').toUpperCase()} · ${input.label ?? ''} — ${input.detail ?? ''}`,
      }
    default:
      return null
  }
}

let checksTurn = -1
let turnSeq = 0

export const useStore = create<AppStore>((set, get) => ({
  view: { kind: 'dashboard' },
  templates: [],
  system: null,
  lab: null,
  states: {},
  consoles: [],
  activeConsole: null,
  eventsSocket: null,

  agentItems: [],
  agentBusy: false,
  agentConnected: false,
  agentSocket: null,

  inspectorTab: 'console',
  inspectorNode: null,
  checks: [],
  lastValidated: null,
  sessions: [],
  cfgTouched: {},

  openDashboard() {
    get().agentSocket?.close()
    set({
      view: { kind: 'dashboard' },
      lab: null,
      states: {},
      consoles: [],
      activeConsole: null,
      agentItems: [],
      agentBusy: false,
      agentConnected: false,
      agentSocket: null,
      inspectorTab: 'console',
      inspectorNode: null,
      checks: [],
      lastValidated: null,
      sessions: [],
      cfgTouched: {},
    })
  },

  async openLab(labId: string) {
    const lab = await api.lab(labId)
    set({
      view: { kind: 'lab', labId },
      lab,
      states: lab.states,
      consoles: [],
      activeConsole: null,
      inspectorTab: 'console',
      inspectorNode: null,
      sessions: [],
    })
    get().connectEvents()
    get().connectAgent(labId)
  },

  async refreshLab() {
    const view = get().view
    if (view.kind !== 'lab') return
    const lab = await api.lab(view.labId)
    // Keep only states for nodes that still exist (deleted nodes must not
    // pin the header at "running"), letting live WS values win over the
    // snapshot; drop console tabs whose node is gone.
    const states: Record<string, NodeState> = { ...lab.states }
    for (const [id, st] of Object.entries(get().states)) {
      if (lab.nodes[id]) states[id] = st
    }
    const consoles = get().consoles.filter((c) => lab.nodes[c.nodeId])
    const active = get().activeConsole
    set({
      lab,
      states,
      consoles,
      activeConsole: active && lab.nodes[active] ? active : consoles[0]?.nodeId ?? null,
    })
  },

  async loadTemplates() {
    const templates = await api.templates()
    set({ templates })
  },

  async loadSystem() {
    try {
      set({ system: await api.system() })
    } catch {
      /* header degrades gracefully */
    }
  },

  connectEvents() {
    if (get().eventsSocket) return
    const ws = new WebSocket(wsUrl('/api/ws/events'))
    ws.onmessage = (msg) => {
      try {
        const ev = JSON.parse(msg.data)
        const view = get().view
        if (ev.type === 'node_state') {
          // The events socket is a global all-labs broadcast; ignore other
          // labs' transitions so states and the audit log stay per-lab.
          if (view.kind !== 'lab' || (ev.lab && ev.lab !== view.labId)) return
          const prev = get().states[ev.node]
          set((s) => ({ states: { ...s.states, [ev.node]: ev.state } }))
          if (ev.detail) get().pushLog(ev.state === 'error' ? 'error' : 'info', `${ev.state}: ${ev.detail}`)
          if (prev !== ev.state && (ev.state === 'running' || ev.state === 'error' || ev.state === 'stopped')) {
            const name = get().lab?.nodes[ev.node]?.name ?? ev.node
            get().record(ev.state === 'error' ? 'error' : 'deploy', `${name} is ${ev.state}`)
          }
        } else if (ev.type === 'lab_updated') {
          if (view.kind === 'lab' && view.labId === ev.lab) {
            void get().refreshLab()
          }
        } else if (ev.type === 'log') {
          get().pushLog(ev.level, ev.message)
        }
      } catch {
        /* ignore malformed */
      }
    }
    ws.onclose = () => {
      set({ eventsSocket: null })
      // Reconnect while a lab is open.
      setTimeout(() => {
        if (get().view.kind === 'lab') get().connectEvents()
      }, 2000)
    }
    set({ eventsSocket: ws })
  },

  openConsole(nodeId, nodeName) {
    const { consoles } = get()
    if (!consoles.some((c) => c.nodeId === nodeId)) {
      set({ consoles: [...consoles, { nodeId, nodeName }] })
      const tmpl = get().lab?.nodes[nodeId]?.template
      get().record('ssh', `Opened console to ${nodeName}${tmpl ? ` (${tmpl})` : ''}`)
    }
    set({ activeConsole: nodeId, inspectorTab: 'console' })
  },

  closeConsole(nodeId) {
    const consoles = get().consoles.filter((c) => c.nodeId !== nodeId)
    const active = get().activeConsole === nodeId ? consoles[0]?.nodeId ?? null : get().activeConsole
    set({ consoles, activeConsole: active })
  },

  setActiveConsole(nodeId) {
    set({ activeConsole: nodeId })
  },

  connectAgent(labId: string, fresh = true) {
    const existing = get().agentSocket
    if (existing && existing.readyState <= WebSocket.OPEN) existing.close()
    const ws = new WebSocket(wsUrl(`/api/ws/agent/${labId}`))
    ws.onopen = () => {
      if (get().agentSocket === ws) set({ agentConnected: true })
    }
    ws.onclose = () => {
      if (get().agentSocket === ws) set({ agentConnected: false, agentBusy: false, agentSocket: null })
    }
    ws.onmessage = (msg) => {
      if (get().agentSocket !== ws) return // stale socket must not touch state
      try {
        const ev = JSON.parse(msg.data)
        if (ev.type === 'text') {
          set((s) => ({ agentItems: [...s.agentItems, { kind: 'assistant', text: ev.text }] }))
          get().record('agent', ev.text)
        } else if (ev.type === 'tool_call') {
          const input = (ev.input ?? {}) as AgentToolInput
          if (ev.name === 'report_check') {
            const check: Check = {
              label: input.label ?? 'check',
              status: normalizeCheckStatus(input.status),
              detail: input.detail ?? '',
              at: Date.now(),
            }
            set((s) => ({
              checks: checksTurn === turnSeq ? [...s.checks, check] : [check],
              lastValidated: Date.now(),
            }))
            checksTurn = turnSeq
          }
          if (ev.name === 'set_startup_config' && input.node) {
            set((s) => ({ cfgTouched: { ...s.cfgTouched, [input.node!]: Date.now() } }))
          }
          const entry = toolSessionEntry(ev.name, input)
          if (entry) get().record(entry.kind, entry.text)
          set((s) => ({
            agentItems: [...s.agentItems, { kind: 'tool', id: ev.id, name: ev.name, input: ev.input }],
          }))
        } else if (ev.type === 'tool_result') {
          set((s) => {
            const items = [...s.agentItems]
            const idx = items.findIndex((i) => i.kind === 'tool' && i.id === ev.id)
            if (idx >= 0) {
              const t = items[idx] as Extract<ChatItem, { kind: 'tool' }>
              items[idx] = { ...t, output: ev.output, isError: ev.is_error }
            }
            return { agentItems: items }
          })
          // No refreshLab here: every mutating tool already triggers a
          // lab_updated event, which refreshes exactly once.
        } else if (ev.type === 'error') {
          set((s) => ({ agentItems: [...s.agentItems, { kind: 'error', text: ev.message }], agentBusy: false }))
          get().record('error', ev.message)
        } else if (ev.type === 'done') {
          // Close the turn: post a validation report card when the agent
          // recorded checks during this turn.
          set((s) => {
            const items = [...s.agentItems]
            if (checksTurn === turnSeq && s.checks.length) {
              const passed = s.checks.filter((c) => c.status === 'pass').length
              const warn = s.checks.filter((c) => c.status === 'warn').length
              const fail = s.checks.filter((c) => c.status === 'fail').length
              items.push({ kind: 'report', passed, warn, fail, total: s.checks.length })
            }
            return { agentItems: items, agentBusy: false }
          })
          void get().refreshLab()
        }
      } catch {
        /* ignore */
      }
    }
    // `fresh` = new lab context; a plain reconnect keeps the transcript.
    checksTurn = -1
    if (fresh) {
      set({
        agentSocket: ws,
        agentItems: [],
        agentBusy: false,
        checks: [],
        lastValidated: null,
        cfgTouched: {},
      })
    } else {
      set({ agentSocket: ws, agentBusy: false })
    }
  },

  sendAgent(message: string, files?: string[]) {
    const ws = get().agentSocket
    if (!message.trim() || get().agentBusy || !ws || ws.readyState !== WebSocket.OPEN) return false
    turnSeq += 1
    set((s) => ({
      agentItems: [...s.agentItems, { kind: 'user', text: message, files }],
      agentBusy: true,
    }))
    get().record('prompt', files?.length ? `${message} [${files.join(', ')}]` : message)
    ws.send(JSON.stringify({ message }))
    return true
  },

  setInspectorTab(tab) {
    set({ inspectorTab: tab })
  },

  setInspectorNode(nodeId) {
    set({ inspectorNode: nodeId })
  },

  record(kind, text) {
    const t = text.length > 500 ? text.slice(0, 500) + '…' : text
    set((s) => ({ sessions: [...s.sessions, { ts: Date.now(), kind, text: t }].slice(-500) }))
  },

  clearSessions() {
    set({ sessions: [] })
  },

  pushLog(level, message) {
    get().record(level === 'error' ? 'error' : level === 'warn' ? 'warn' : 'sys', message)
  },
}))

// Dev-only debug handle for inspecting or driving the store from the
// console; stripped from production builds.
declare global {
  interface Window {
    __strato?: typeof useStore
  }
}
if (import.meta.env.DEV && typeof window !== 'undefined') window.__strato = useStore
