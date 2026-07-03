// Topology canvas built on React Flow, rendered in the Strato visual
// language: device cards with vendor-hued icon tiles, thin straight links
// with subnet labels and animated packet dots, dashed-red failed links.
// Editing behaviors (drag, cable, context menu, drop-to-create) are kept.

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
  getStraightPath,
  useReactFlow,
  type Connection,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from '@xyflow/react'
import { api, ifaceName, type Endpoint, type LabView, type Template } from '../api'
import { useStore } from '../store'
import { deviceIcon, hue, hueBg, hueOf } from '../vendors'
import { stateColor } from './icons'

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

const mono = "'IBM Plex Mono', ui-monospace, monospace"

const pulse = 'pulseDot 1.2s ease-in-out infinite'
const stateDot = (state: string): { color: string; anim: string } => ({
  color: stateColor[state] ?? stateColor.stopped,
  anim: state === 'starting' || state === 'stopping' ? pulse : 'none',
})

// ---------- custom node renderers ----------

function DeviceNode({ data, selected }: NodeProps) {
  const d = data as {
    name: string
    icon: string
    templateName: string
    vendor: string
    state: string
  }
  const h = hueOf(d.vendor)
  const dot = stateDot(d.state)
  return (
    <div style={{ animation: 'popIn .45s cubic-bezier(.2,1.2,.4,1)' }}>
      <div
        className="nodecard"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          background: 'var(--panel)',
          border: `1.5px solid ${selected ? 'var(--accent)' : 'var(--border)'}`,
          borderRadius: 12,
          padding: '9px 14px 9px 9px',
          boxShadow: selected ? '0 0 0 3px var(--accentSoft), var(--shadow)' : 'var(--shadow)',
          minWidth: 138,
        }}
      >
        <div
          style={{
            width: 34,
            height: 34,
            borderRadius: 9,
            background: hueBg(h),
            display: 'grid',
            placeItems: 'center',
            flex: '0 0 34px',
          }}
        >
          {deviceIcon(d.icon, 18, hue(h))}
        </div>
        <div>
          <div
            style={{
              fontSize: 12.5,
              fontWeight: 600,
              letterSpacing: '-.1px',
              whiteSpace: 'nowrap',
              fontFamily: mono,
              color: 'var(--text)',
            }}
          >
            {d.name}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginTop: 2 }}>
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: dot.color,
                animation: dot.anim,
              }}
            />
            <span style={{ fontSize: 10.5, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
              {d.templateName}
            </span>
          </div>
        </div>
      </div>
      <Handle
        type="source"
        position={Position.Top}
        id="h"
        style={{ top: 26, left: 26, transform: 'translate(-50%,-50%)' }}
      />
    </div>
  )
}

function NetworkNode({ data, selected }: NodeProps) {
  const d = data as { name: string; netKind: string }
  const iconName = d.netKind === 'cloud' ? 'cloud' : d.netKind === 'nat' ? 'internet' : 'network'
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 4,
        animation: 'popIn .45s cubic-bezier(.2,1.2,.4,1)',
      }}
    >
      <div
        style={{
          width: 40,
          height: 40,
          borderRadius: '50%',
          border: `1.5px dashed ${selected ? 'var(--accent)' : 'var(--border2)'}`,
          background: 'var(--panel)',
          display: 'grid',
          placeItems: 'center',
          boxShadow: selected ? '0 0 0 3px var(--accentSoft)' : 'none',
        }}
      >
        {deviceIcon(iconName, 18, selected ? 'var(--accent)' : 'var(--muted)')}
      </div>
      <div style={{ fontSize: 10, fontFamily: mono, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
        {d.name}
      </div>
      <div style={{ fontSize: 8, textTransform: 'uppercase', letterSpacing: '.5px', color: 'var(--muted)', opacity: 0.7, marginTop: -3 }}>
        {d.netKind}
      </div>
      <Handle
        type="source"
        position={Position.Top}
        id="h"
        style={{ top: 20, left: '50%', transform: 'translate(-50%,-50%)' }}
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
        className={`whitespace-pre-wrap px-1 ${selected ? 'ring-1 ring-accent-500' : ''}`}
        style={{ color: d.color || '#8a94a6', fontSize: d.font_size || 14 }}
      >
        {d.text || 'text'}
      </div>
    )
  }
  return (
    <div
      className={selected ? 'ring-1 ring-accent-500' : ''}
      style={{
        width: d.width || 160,
        height: d.height || 100,
        background: d.fill || 'rgba(56,209,186,0.05)',
        border: `1.5px solid ${d.color || '#2f3846'}`,
        borderRadius: d.annKind === 'ellipse' ? '50%' : 8,
      }}
    />
  )
}

// ---------- Strato edge: straight line, subnet label, packet dot ----------

function StratoEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY } = props
  const [path, labelX, labelY] = getStraightPath({ sourceX, sourceY, targetX, targetY })
  const data = props.data as
    | {
        aLabel?: string
        bLabel?: string
        label?: string
        impaired?: boolean
        suspended?: boolean
        active?: boolean
      }
    | undefined
  const down = !!data?.suspended
  const len = Math.hypot(targetX - sourceX, targetY - sourceY)
  const lerp = (t: number) => ({
    x: sourceX + (targetX - sourceX) * t,
    y: sourceY + (targetY - sourceY) * t,
  })
  const pa = lerp(0.25)
  const pb = lerp(0.75)
  return (
    <>
      <BaseEdge
        {...props}
        path={path}
        style={{
          stroke: props.selected
            ? 'var(--accent)'
            : down
              ? 'var(--red)'
              : data?.impaired
                ? 'var(--amber)'
                : 'var(--border2)',
          strokeWidth: down ? 2 : 1.6,
          strokeDasharray: down ? '7 6' : data?.impaired ? '6 3' : undefined,
        }}
      />
      {down && (
        <g transform={`translate(${labelX},${labelY})`} style={{ pointerEvents: 'none' }}>
          <circle r={9} fill="var(--bg)" stroke="var(--red)" strokeWidth={1.5} />
          <path
            d="M-3.5 -3.5 L3.5 3.5 M3.5 -3.5 L-3.5 3.5"
            stroke="var(--red)"
            strokeWidth={1.8}
            strokeLinecap="round"
          />
        </g>
      )}
      {!down && data?.active && (
        <circle r={2.8} fill="var(--accent)" style={{ pointerEvents: 'none' }}>
          <animateMotion
            dur={`${Math.max(len / 130, 0.8).toFixed(2)}s`}
            repeatCount="indefinite"
            path={`M ${sourceX} ${sourceY} L ${targetX} ${targetY}`}
          />
        </circle>
      )}
      {!down && data?.label && (
        <text
          x={labelX}
          y={labelY - 7}
          fill="var(--muted)"
          fontSize={9.5}
          fontFamily={mono}
          textAnchor="middle"
          opacity={0.85}
          style={{ pointerEvents: 'none' }}
        >
          {data.label}
        </text>
      )}
      <EdgeLabelRenderer>
        {data?.aLabel && (
          <span
            className="absolute rounded px-1 font-mono text-[8.5px]"
            style={{
              transform: `translate(-50%,-50%) translate(${pa.x}px,${pa.y}px)`,
              background: 'rgba(12,14,17,0.85)',
              color: 'var(--muted)',
            }}
          >
            {data.aLabel}
          </span>
        )}
        {data?.bLabel && (
          <span
            className="absolute rounded px-1 font-mono text-[8.5px]"
            style={{
              transform: `translate(-50%,-50%) translate(${pb.x}px,${pb.y}px)`,
              background: 'rgba(12,14,17,0.85)',
              color: 'var(--muted)',
            }}
          >
            {data.bLabel}
          </span>
        )}
      </EdgeLabelRenderer>
    </>
  )
}

const nodeTypes = { device: DeviceNode, net: NetworkNode, annotation: AnnotationNode }
const edgeTypes = { strato: StratoEdge }

// ---------- graph derivation ----------

function endpointNodeId(ep: Endpoint): string {
  return ep.kind === 'node' ? ep.node : ep.network
}

function buildGraph(
  lab: LabView,
  states: Record<string, string>,
  templates: Template[],
): { nodes: Node[]; edges: Edge[] } {
  const byId = new Map(templates.map((t) => [t.id, t]))
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
    const t = byId.get(n.template)
    nodes.push({
      id: n.id,
      type: 'device',
      position: { x: n.x, y: n.y },
      data: {
        name: n.name,
        icon: n.icon,
        templateName: t?.name ?? n.template,
        vendor: t?.vendor ?? '',
        state: states[n.id] ?? 'stopped',
      },
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
  const running = (id: string) => (states[id] ?? 'stopped') === 'running'
  const edges: Edge[] = Object.values(lab.links).map((l) => {
    const label = (ep: Endpoint): string | undefined => {
      if (ep.kind !== 'node') return undefined
      const node = lab.nodes[ep.node]
      if (!node) return undefined
      return ifaceName(byId.get(node.template)?.iface_pattern ?? 'eth{i}', ep.iface)
    }
    const epActive = (ep: Endpoint) => (ep.kind === 'node' ? running(ep.node) : true)
    return {
      id: l.id,
      type: 'strato',
      source: endpointNodeId(l.a),
      target: endpointNodeId(l.b),
      data: {
        aLabel: label(l.a),
        bLabel: label(l.b),
        label: l.label,
        suspended: !!l.suspended,
        active: epActive(l.a) && epActive(l.b) && !l.suspended,
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

  const { nodes, edges } = useMemo(
    () => (lab ? buildGraph(lab, states, templates) : { nodes: [], edges: [] }),
    [lab, states, templates],
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
  const nodeCount = Object.keys(lab.nodes).length
  const linkCount = Object.keys(lab.links).length
  const anyRunning = Object.values(states).some((s) => s === 'running')

  return (
    <div
      ref={wrapper}
      className="relative h-full w-full"
      onClick={() => setMenu(null)}
      style={{
        background: 'var(--bg)',
        backgroundImage: 'radial-gradient(#1a212c 1.2px, transparent 1.2px)',
        backgroundSize: '26px 26px',
      }}
    >
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
          if (!n) return
          const st = states[node.id] ?? 'stopped'
          if (st === 'running' || st === 'starting') openConsole(n.id, n.name)
          else pushLog('info', `${n.name} is not running — start it to open its console`)
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
        <Background variant={BackgroundVariant.Dots} gap={26} size={1.2} color="#1a212c" />
        <Controls position="bottom-left" showInteractive={false} style={{ marginBottom: 46 }} />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          nodeColor={() => '#2f3846'}
          maskColor="rgba(12,14,17,0.72)"
          style={{ width: 140, height: 90 }}
        />
      </ReactFlow>

      {nodeCount === 0 && (
        <div className="pointer-events-none absolute inset-0 z-10 grid place-items-center">
          <div style={{ textAlign: 'center', color: 'var(--muted)' }}>
            <svg
              width={44}
              height={44}
              viewBox="0 0 24 24"
              fill="none"
              stroke="var(--border2)"
              strokeWidth={1.4}
              style={{ margin: '0 auto 10px' }}
            >
              <circle cx={5.5} cy={6} r={2.5} />
              <circle cx={18.5} cy={6} r={2.5} />
              <circle cx={12} cy={18} r={2.5} />
              <path d="M7.5 7.5 10.5 16M16.5 7.5 13.5 16M8 6h8" />
            </svg>
            <div style={{ fontSize: 13 }}>The lab you describe will build itself here.</div>
            <div style={{ fontSize: 11.5, marginTop: 4, opacity: 0.75 }}>
              Ask the agent on the left — or open Devices and drag one on.
            </div>
          </div>
        </div>
      )}

      <div className="absolute bottom-3.5 left-1/2 z-10 flex -translate-x-1/2 gap-2">
        <span
          style={{
            fontSize: 11,
            color: 'var(--muted)',
            background: 'var(--panel)',
            border: '1px solid var(--border)',
            borderRadius: 8,
            padding: '6px 11px',
            whiteSpace: 'nowrap',
          }}
        >
          {nodeCount} devices · {linkCount} links
        </span>
        {anyRunning && (
          <span
            style={{
              fontSize: 11,
              color: 'var(--muted)',
              background: 'var(--panel)',
              border: '1px solid var(--border)',
              borderRadius: 8,
              padding: '6px 11px',
              whiteSpace: 'nowrap',
            }}
          >
            Double-click a device for its console
          </span>
        )}
      </div>

      {menu && menuNode && (
        <div
          className="absolute z-50 w-44 overflow-hidden rounded-xl border border-ink-800 bg-ink-900 py-1"
          style={{ left: menu.x, top: menu.y, boxShadow: 'var(--shadow)' }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="border-b border-ink-800 px-3 py-1.5 font-mono text-xs text-ink-300">
            {menuNode.name}
            <span className="ml-2 text-[10px]" style={{ color: stateDot(menuState).color }}>
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
        danger ? 'text-[#f0655a] hover:bg-red-950/40' : 'text-ink-200 hover:bg-ink-850'
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
