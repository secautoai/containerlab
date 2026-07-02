// Zustand app store: current view, lab document, runtime states, consoles.

import { create } from 'zustand'
import {
  api,
  type LabView,
  type NodeState,
  type Template,
} from './api'

export interface ConsoleTab {
  nodeId: string
  nodeName: string
}

export interface LogLine {
  level: string
  message: string
  at: number
}

interface AppStore {
  view: { kind: 'dashboard' } | { kind: 'lab'; labId: string }
  templates: Template[]
  lab: LabView | null
  states: Record<string, NodeState>
  consoles: ConsoleTab[]
  activeConsole: string | null
  consoleOpen: boolean
  agentOpen: boolean
  logs: LogLine[]
  eventsSocket: WebSocket | null

  openDashboard(): void
  openLab(labId: string): Promise<void>
  refreshLab(): Promise<void>
  loadTemplates(): Promise<void>
  connectEvents(): void
  openConsole(nodeId: string, nodeName: string): void
  closeConsole(nodeId: string): void
  setActiveConsole(nodeId: string): void
  toggleAgent(): void
  setConsoleOpen(open: boolean): void
  pushLog(level: string, message: string): void
}

export const useStore = create<AppStore>((set, get) => ({
  view: { kind: 'dashboard' },
  templates: [],
  lab: null,
  states: {},
  consoles: [],
  activeConsole: null,
  consoleOpen: false,
  agentOpen: false,
  logs: [],
  eventsSocket: null,

  openDashboard() {
    set({
      view: { kind: 'dashboard' },
      lab: null,
      states: {},
      consoles: [],
      activeConsole: null,
      consoleOpen: false,
    })
  },

  async openLab(labId: string) {
    const lab = await api.lab(labId)
    set({ view: { kind: 'lab', labId }, lab, states: lab.states })
    get().connectEvents()
  },

  async refreshLab() {
    const view = get().view
    if (view.kind !== 'lab') return
    const lab = await api.lab(view.labId)
    set({ lab, states: { ...get().states, ...lab.states } })
  },

  async loadTemplates() {
    const templates = await api.templates()
    set({ templates })
  },

  connectEvents() {
    if (get().eventsSocket) return
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    const ws = new WebSocket(`${proto}://${location.host}/api/ws/events`)
    ws.onmessage = (msg) => {
      try {
        const ev = JSON.parse(msg.data)
        const view = get().view
        if (ev.type === 'node_state') {
          set((s) => ({ states: { ...s.states, [ev.node]: ev.state } }))
          if (ev.detail) get().pushLog(ev.state === 'error' ? 'error' : 'info', `${ev.state}: ${ev.detail}`)
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
    }
    set({ activeConsole: nodeId, consoleOpen: true })
  },

  closeConsole(nodeId) {
    const consoles = get().consoles.filter((c) => c.nodeId !== nodeId)
    const active = get().activeConsole === nodeId ? consoles[0]?.nodeId ?? null : get().activeConsole
    set({ consoles, activeConsole: active, consoleOpen: consoles.length > 0 && get().consoleOpen })
  },

  setActiveConsole(nodeId) {
    set({ activeConsole: nodeId })
  },

  toggleAgent() {
    set((s) => ({ agentOpen: !s.agentOpen }))
  },

  setConsoleOpen(open) {
    set({ consoleOpen: open })
  },

  pushLog(level, message) {
    set((s) => ({ logs: [...s.logs.slice(-199), { level, message, at: Date.now() }] }))
  },
}))
