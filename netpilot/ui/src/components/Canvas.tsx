// Topology canvas built on React Flow: device/network/annotation nodes,
// interface-labeled edges, drag-drop node creation, context menus.

import { useCallback, useMemo, useRef, useState } from 'react'
import {
  Background,
  BackgroundVariant,
  BaseEdge,
  Controls,
  EdgeLabelRenderer,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  getBezierPath,
  useReactFlow,
  type Connection,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from '@xyflow/react'
import { Cloud, Globe, Network as NetworkIcon, Waypoints } from 'lucide-react'
import { api, ifaceName, type Endpoint, type LabView } from '../api'
import { useStore } from '../store'
import { iconFor, stateColor } from './icons'

export interface Selection {
  kind: 'node' | 'network' | 'link' | 'annotation'
  id: string
}

interface CanvasProps {
  onSelect: (sel: Selection | null) => void
}

interface MenuState {
  x: number
  y: number
  nodeId: string
}

// ---------- custom node renderers ----------

function DeviceNode({ data, selected }: NodeProps) {
  const d = data as { name: string; icon: string; template: string; state: string }
  const Icon = iconFor(d.icon)
  const color = stateColor[d.state] ?? stateColor.stopped
  return (
    <div
      className={`flex w-24 flex-col items-center gap-1 rounded-lg px-2 py-2 transition ${
        selected ? 'bg-ink-800 ring-1 ring-cyan-400' : 'hover:bg-ink-850'
      }`}
    >
      <div
        className="relative flex h-12 w-12 items-center justify-center rounded-xl border-2 bg-ink-850"
        style={{ borderColor: color }}
      >
        <Icon size={24} style={{ color }} />
        <span
          className="absolute -right-1 -top-1 h-3 w-3 rounded-full border-2 border-ink-950"
          style={{ background: color }}
        />
      </div>
      <span className="max-w-24 truncate font-mono text-[11px] text-ink-200">{d.name}</span>
      <span className="-mt-1 text-[9px] uppercase tracking-wide text-ink-600">{d.template}</span>
      <Handle
        type="source"
        position={Position.Top}
        id="h"
        style={{ top: 32, left: '50%', transform: 'translate(-50%,-50%)' }}
      />
    </div>
  )
}

function NetworkNode({ data, selected }: NodeProps) {
  const d = data as { name: string; netKind: string }
  const Icon =
    d.netKind === 'cloud' ? Cloud : d.netKind === 'nat' ? Globe : d.netKind === 'management' ? Waypoints : NetworkIcon
  return (
    <div
      className={`flex flex-col items-center gap-1 rounded-lg px-2 py-1.5 ${
        selected ? 'bg-ink-800 ring-1 ring-cyan-400' : 'hover:bg-ink-850'
      }`}
    >
      <div className="flex h-9 w-9 items-center justify-center rounded-full border border-dashed border-ink-400 bg-ink-850 text-ink-300">
        <Icon size={18} />
      </div>
      <span className="font-mono text-[10px] text-ink-300">{d.name}</span>
      <span className="-mt-1 text-[8px] uppercase text-ink-600">{d.netKind}</span>
      <Handle
        type="source"
        position={Position.Top}
        id="h"
        style={{ top: 24, left: '50%', transform: 'translate(-50%,-50%)' }}
      />
    </div>
  )
}

function AnnotationNode({ data, selected }: NodeProps) {
  const d = data as {
    annKind: string
    text: string
    color: string
    fill: string
    font_size: number
    width: number
    height: number
  }
  if (d.annKind === 'text') {
    return (
      <div
        className={`whitespace-pre-wrap px-1 ${selected ? 'ring-1 ring-cyan-400' : ''}`}
        style={{ color: d.color || '#94a3b8', fontSize: d.font_size || 14 }}
      >
        {d.text || 'text'}
      </div>
    )
  }
  return (
    <div
      className={selected ? 'ring-1 ring-cyan-400' : ''}
      style={{
        width: d.width || 160,
        height: d.height || 100,
        background: d.fill || 'rgba(34,211,238,0.06)',
        border: `1.5px solid ${d.color || '#334155'}`,
        borderRadius: d.annKind === 'ellipse' ? '50%' : 8,
      }}
    />
  )
}

// ---------- custom edge with per-end interface labels ----------

function LabelledEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition } = props
  const [path] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    curvature: 0.2,
  })
  const data = props.data as
    | { aLabel?: string; bLabel?: string; impaired?: boolean; suspended?: boolean }
    | undefined
  const lerp = (t: number) => ({
    x: sourceX + (targetX - sourceX) * t,
    y: sourceY + (targetY - sourceY) * t,
  })
  const pa = lerp(0.22)
  const pb = lerp(0.78)
  return (
    <>
      <BaseEdge
        {...props}
        path={path}
        style={{
          stroke: props.selected
            ? '#22d3ee'
            : data?.suspended
              ? '#ef4444'
              : data?.impaired
                ? '#f59e0b'
                : '#475569',
          strokeWidth: props.selected ? 2 : 1.5,
          strokeDasharray: data?.suspended ? '3 4' : data?.impaired ? '6 3' : undefined,
          opacity: data?.suspended ? 0.6 : 1,
        }}
      />
      <EdgeLabelRenderer>
        {data?.aLabel && (
          <span
            className="absolute rounded bg-ink-900/90 px-1 font-mono text-[9px] text-ink-300"
            style={{ transform: `translate(-50%,-50%) translate(${pa.x}px,${pa.y}px)` }}
          >
            {data.aLabel}
          </span>
        )}
        {data?.bLabel && (
          <span
            className="absolute rounded bg-ink-900/90 px-1 font-mono text-[9px] text-ink-300"
            style={{ transform: `translate(-50%,-50%) translate(${pb.x}px,${pb.y}px)` }}
          >
            {data.bLabel}
          </span>
        )}
      </EdgeLabelRenderer>
    </>
  )
}

const nodeTypes = { device: DeviceNode, net: NetworkNode, annotation: AnnotationNode }
const edgeTypes = { labelled: LabelledEdge }

// ---------- graph derivation ----------

function endpointNodeId(ep: Endpoint): string {
  return ep.kind === 'node' ? ep.node : ep.network
}

function buildGraph(
  lab: LabView,
  states: Record<string, string>,
  patterns: Record<string, string>,
): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = []
  for (const ann of Object.values(lab.annotations)) {
    nodes.push({
      id: ann.id,
      type: 'annotation',
      position: { x: ann.x, y: ann.y },
      data: {
        annKind: ann.kind,
        text: ann.text,
        color: ann.color,
        fill: ann.fill,
        font_size: ann.font_size,
        width: ann.width,
        height: ann.height,
      },
      zIndex: ann.z < 0 ? -10 : 10,
    })
  }
  for (const n of Object.values(lab.nodes)) {
    nodes.push({
      id: n.id,
      type: 'device',
      position: { x: n.x, y: n.y },
      data: { name: n.name, icon: n.icon, template: n.template, state: states[n.id] ?? 'stopped' },
    })
  }
  for (const net of Object.values(lab.networks)) {
    nodes.push({
      id: net.id,
      type: 'net',
      position: { x: net.x, y: net.y },
      data: { name: net.name, netKind: net.kind },
    })
  }
  const edges: Edge[] = Object.values(lab.links).map((l) => {
    const label = (ep: Endpoint): string | undefined => {
      if (ep.kind !== 'node') return undefined
      const node = lab.nodes[ep.node]
      if (!node) return undefined
      return ifaceName(patterns[node.template] ?? 'eth{i}', ep.iface)
    }
    return {
      id: l.id,
      type: 'labelled',
      source: endpointNodeId(l.a),
      target: endpointNodeId(l.b),
      data: {
        aLabel: label(l.a),
        bLabel: label(l.b),
        suspended: !!l.suspended,
        impaired:
          !!l.impairment &&
          (l.impairment.delay_ms > 0 ||
            l.impairment.loss_pct > 0 ||
            l.impairment.jitter_ms > 0 ||
            l.impairment.rate_kbit > 0),
      },
    }
  })
  return { nodes, edges }
}

/** Lowest interface index not used by any link on this node. */
function freeIface(lab: LabView, nodeId: string): number | null {
  const node = lab.nodes[nodeId]
  if (!node) return null
  const used = new Set<number>()
  for (const l of Object.values(lab.links)) {
    for (const ep of [l.a, l.b]) {
      if (ep.kind === 'node' && ep.node === nodeId) used.add(ep.iface)
    }
  }
  for (let i = 0; i < node.interfaces; i++) {
    if (!used.has(i)) return i
  }
  return null
}

// ---------- the canvas ----------

function CanvasInner({ onSelect }: CanvasProps) {
  const lab = useStore((s) => s.lab)
  const states = useStore((s) => s.states)
  const templates = useStore((s) => s.templates)
  const refreshLab = useStore((s) => s.refreshLab)
  const openConsole = useStore((s) => s.openConsole)
  const pushLog = useStore((s) => s.pushLog)
  const { screenToFlowPosition } = useReactFlow()
  const [menu, setMenu] = useState<MenuState | null>(null)
  const wrapper = useRef<HTMLDivElement>(null)

  const patterns = useMemo(
    () => Object.fromEntries(templates.map((t) => [t.id, t.iface_pattern])),
    [templates],
  )

  const { nodes, edges } = useMemo(
    () => (lab ? buildGraph(lab, states, patterns) : { nodes: [], edges: [] }),
    [lab, states, patterns],
  )

  const act = useCallback(
    async (f: () => Promise<unknown>, errPrefix: string) => {
      try {
        await f()
      } catch (e) {
        pushLog('error', `${errPrefix}: ${e instanceof Error ? e.message : e}`)
      }
      await refreshLab()
    },
    [pushLog, refreshLab],
  )

  const onConnect = useCallback(
    (conn: Connection) => {
      if (!lab || !conn.source || !conn.target || conn.source === conn.target) return
      const endpoint = (id: string): Endpoint | null => {
        if (lab.networks[id]) return { kind: 'network', network: id }
        const iface = freeIface(lab, id)
        if (iface === null) {
          pushLog('error', `${lab.nodes[id]?.name ?? id}: no free interfaces`)
          return null
        }
        return { kind: 'node', node: id, iface }
      }
      const a = endpoint(conn.source)
      // Build b after a so two endpoints on the same node pick distinct ifaces.
      if (!a) return
      const b = endpoint(conn.target)
      if (!b) return
      if (a.kind === 'network' && b.kind === 'network') {
        pushLog('error', 'cannot link two networks directly')
        return
      }
      void act(() => api.createLink(lab.id, { a, b }), 'link')
    },
    [lab, act, pushLog],
  )

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      if (!lab) return
      const templateId = e.dataTransfer.getData('application/netpilot-template')
      if (!templateId) return
      const pos = screenToFlowPosition({ x: e.clientX, y: e.clientY })
      void act(
        () => api.createNode(lab.id, { template: templateId, x: pos.x, y: pos.y }),
        'add node',
      )
    },
    [lab, screenToFlowPosition, act],
  )

  if (!lab) return null

  const menuNode = menu ? lab.nodes[menu.nodeId] : null
  const menuState = menuNode ? (states[menuNode.id] ?? 'stopped') : 'stopped'

  return (
    <div ref={wrapper} className="relative h-full w-full" onClick={() => setMenu(null)}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        connectionMode={'loose' as never}
        fitView
        minZoom={0.2}
        maxZoom={2.5}
        proOptions={{ hideAttribution: true }}
        onConnect={onConnect}
        onDrop={onDrop}
        onDragOver={(e) => {
          e.preventDefault()
          e.dataTransfer.dropEffect = 'copy'
        }}
        onNodeDragStop={(_, node) => {
          const body = { x: node.position.x, y: node.position.y }
          if (lab.nodes[node.id]) void api.updateNode(lab.id, node.id, body)
          else if (lab.networks[node.id]) void api.updateNetwork(lab.id, node.id, body)
          else if (lab.annotations[node.id]) void api.updateAnnotation(lab.id, node.id, body)
        }}
        onNodeClick={(_, node) => {
          const kind = lab.nodes[node.id] ? 'node' : lab.networks[node.id] ? 'network' : 'annotation'
          onSelect({ kind, id: node.id })
        }}
        onNodeDoubleClick={(_, node) => {
          const n = lab.nodes[node.id]
          if (n) openConsole(n.id, n.name)
        }}
        onEdgeClick={(_, edge) => onSelect({ kind: 'link', id: edge.id })}
        onPaneClick={() => {
          onSelect(null)
          setMenu(null)
        }}
        onNodeContextMenu={(e, node) => {
          e.preventDefault()
          if (!lab.nodes[node.id]) return
          const rect = wrapper.current?.getBoundingClientRect()
          setMenu({
            x: e.clientX - (rect?.left ?? 0),
            y: e.clientY - (rect?.top ?? 0),
            nodeId: node.id,
          })
        }}
        onNodesDelete={(deleted) => {
          for (const n of deleted) {
            if (lab.nodes[n.id]) void act(() => api.deleteNode(lab.id, n.id), 'delete node')
            else if (lab.networks[n.id]) void act(() => api.deleteNetwork(lab.id, n.id), 'delete network')
            else if (lab.annotations[n.id]) void act(() => api.deleteAnnotation(lab.id, n.id), 'delete annotation')
          }
          onSelect(null)
        }}
        onEdgesDelete={(deleted) => {
          for (const e of deleted) void act(() => api.deleteLink(lab.id, e.id), 'delete link')
          onSelect(null)
        }}
      >
        <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#1e293b" />
        <Controls position="bottom-left" showInteractive={false} />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          nodeColor={(n) => (n.type === 'device' ? '#334155' : '#1e293b')}
          maskColor="rgba(10,14,20,0.7)"
        />
      </ReactFlow>

      {menu && menuNode && (
        <div
          className="absolute z-50 w-44 overflow-hidden rounded-lg border border-ink-700 bg-ink-850 py-1 shadow-xl"
          style={{ left: menu.x, top: menu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="border-b border-ink-700 px-3 py-1.5 font-mono text-xs text-ink-300">
            {menuNode.name}
            <span className="ml-2 text-[10px]" style={{ color: stateColor[menuState] }}>
              {menuState}
            </span>
          </div>
          {(menuState === 'stopped' || menuState === 'error') && (
            <MenuItem
              label="Start"
              onClick={() => {
                void act(() => api.startNode(lab.id, menuNode.id), 'start')
                setMenu(null)
              }}
            />
          )}
          {menuState === 'running' && (
            <>
              <MenuItem
                label="Open console"
                onClick={() => {
                  openConsole(menuNode.id, menuNode.name)
                  setMenu(null)
                }}
              />
              <MenuItem
                label="Stop"
                onClick={() => {
                  void act(() => api.stopNode(lab.id, menuNode.id), 'stop')
                  setMenu(null)
                }}
              />
            </>
          )}
          <MenuItem
            label="Wipe (factory reset)"
            onClick={() => {
              if (confirm(`Wipe ${menuNode.name}? Its disk changes are lost.`)) {
                void act(() => api.wipeNode(lab.id, menuNode.id), 'wipe')
              }
              setMenu(null)
            }}
          />
          <MenuItem
            danger
            label="Delete"
            onClick={() => {
              if (confirm(`Delete ${menuNode.name} and its links?`)) {
                void act(() => api.deleteNode(lab.id, menuNode.id), 'delete')
                onSelect(null)
              }
              setMenu(null)
            }}
          />
        </div>
      )}
    </div>
  )
}

function MenuItem({
  label,
  onClick,
  danger,
}: {
  label: string
  onClick: () => void
  danger?: boolean
}) {
  return (
    <button
      onClick={onClick}
      className={`block w-full px-3 py-1.5 text-left text-sm ${
        danger ? 'text-red-400 hover:bg-red-950/60' : 'text-ink-200 hover:bg-ink-700'
      }`}
    >
      {label}
    </button>
  )
}

export default function Canvas(props: CanvasProps) {
  return (
    <ReactFlowProvider>
      <CanvasInner {...props} />
    </ReactFlowProvider>
  )
}
