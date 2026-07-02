// REST client + shared types mirroring the Rust domain model.

export type NodeState = 'stopped' | 'starting' | 'running' | 'stopping' | 'error'
export type ConsoleKind = 'serial' | 'vnc'
export type NetworkKind = 'bridge' | 'nat' | 'management' | 'cloud'
export type AnnotationKind = 'text' | 'rect' | 'ellipse'

export interface LabSummary {
  id: string
  name: string
  description: string
  folder: string
  node_count: number
  modified_at: string
}

export interface LabNode {
  id: string
  name: string
  template: string
  image: string
  cpus: number
  ram_mb: number
  interfaces: number
  console: ConsoleKind
  icon: string
  x: number
  y: number
  startup_config?: string
  boot_delay_s: number
}

export interface Network {
  id: string
  name: string
  kind: NetworkKind
  host_interface?: string
  subnet?: string
  x: number
  y: number
}

export type Endpoint =
  | { kind: 'node'; node: string; iface: number }
  | { kind: 'network'; network: string }

export interface Impairment {
  delay_ms: number
  jitter_ms: number
  loss_pct: number
  rate_kbit: number
}

export interface Link {
  id: string
  a: Endpoint
  b: Endpoint
  label?: string
  impairment?: Impairment
  suspended?: boolean
}

export interface Annotation {
  id: string
  kind: AnnotationKind
  x: number
  y: number
  width: number
  height: number
  text: string
  color: string
  fill: string
  font_size: number
  z: number
}

export interface Lab {
  id: string
  name: string
  description: string
  author: string
  folder: string
  body?: string
  locked?: boolean
  created_at: string
  modified_at: string
  nodes: Record<string, LabNode>
  networks: Record<string, Network>
  links: Record<string, Link>
  annotations: Record<string, Annotation>
}

export interface LabView extends Lab {
  states: Record<string, NodeState>
  kvm: boolean
}

export interface QemuSpec {
  arch: string
  nic_model: string
  disk_bus: string
  machine: string
  cpu_model: string
  mgmt_nic: boolean
}

export interface Template {
  id: string
  name: string
  vendor: string
  icon: string
  cpus: number
  ram_mb: number
  interfaces: number
  max_interfaces: number
  iface_pattern: string
  console: ConsoleKind
  qemu: QemuSpec
  notes: string
  available_images: string[]
}

export interface SystemStatus {
  version: string
  kvm: boolean
  qemu_available: boolean
  running_nodes: number
  labs: number
  images: number
}

export interface DiskImage {
  template: string
  version: string
  path: string
  size_bytes: number
}

export interface InterfaceView {
  index: number
  name: string
  connected: boolean
  link: string | null
}

class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined ? { 'content-type': 'application/json' } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
  const text = await res.text()
  if (!res.ok) {
    let message = text
    try {
      message = JSON.parse(text).error ?? text
    } catch {
      /* raw */
    }
    throw new ApiError(res.status, message)
  }
  return text ? JSON.parse(text) : (undefined as T)
}

export const api = {
  system: () => request<SystemStatus>('GET', '/api/system'),
  templates: () => request<Template[]>('GET', '/api/templates'),
  images: () => request<DiskImage[]>('GET', '/api/images'),

  labs: () => request<LabSummary[]>('GET', '/api/labs'),
  createLab: (body: { name: string; description?: string }) =>
    request<Lab>('POST', '/api/labs', body),
  lab: (id: string) => request<LabView>('GET', `/api/labs/${id}`),
  updateLab: (id: string, body: Partial<Pick<Lab, 'name' | 'description' | 'folder' | 'body'>>) =>
    request<Lab>('PUT', `/api/labs/${id}`, body),
  setLock: (id: string, locked: boolean) =>
    request<Lab>('PUT', `/api/labs/${id}/lock`, { locked }),
  labStats: (id: string) =>
    request<{ node: string; rss_mb: number; cpu_seconds: number }[]>(
      'GET',
      `/api/labs/${id}/stats`,
    ),
  deleteLab: (id: string) => request<unknown>('DELETE', `/api/labs/${id}`),
  cloneLab: (id: string) => request<Lab>('POST', `/api/labs/${id}/clone`),
  startLab: (id: string) => request<unknown>('POST', `/api/labs/${id}/start`),
  stopLab: (id: string) => request<unknown>('POST', `/api/labs/${id}/stop`),

  createNode: (
    lab: string,
    body: { template: string; name?: string; x?: number; y?: number },
  ) => request<LabNode>('POST', `/api/labs/${lab}/nodes`, body),
  updateNode: (lab: string, node: string, body: Partial<LabNode>) =>
    request<LabNode>('PUT', `/api/labs/${lab}/nodes/${node}`, body),
  deleteNode: (lab: string, node: string) =>
    request<unknown>('DELETE', `/api/labs/${lab}/nodes/${node}`),
  startNode: (lab: string, node: string) =>
    request<unknown>('POST', `/api/labs/${lab}/nodes/${node}/start`),
  stopNode: (lab: string, node: string) =>
    request<unknown>('POST', `/api/labs/${lab}/nodes/${node}/stop`),
  wipeNode: (lab: string, node: string) =>
    request<unknown>('POST', `/api/labs/${lab}/nodes/${node}/wipe`),
  nodeConfig: (lab: string, node: string) =>
    request<{ config: string }>('GET', `/api/labs/${lab}/nodes/${node}/config`),
  setNodeConfig: (lab: string, node: string, config: string) =>
    request<unknown>('PUT', `/api/labs/${lab}/nodes/${node}/config`, { config }),
  interfaces: (lab: string, node: string) =>
    request<InterfaceView[]>('GET', `/api/labs/${lab}/nodes/${node}/interfaces`),

  createNetwork: (lab: string, body: { name?: string; kind: NetworkKind; x?: number; y?: number }) =>
    request<Network>('POST', `/api/labs/${lab}/networks`, body),
  updateNetwork: (lab: string, net: string, body: Partial<Network>) =>
    request<Network>('PUT', `/api/labs/${lab}/networks/${net}`, body),
  deleteNetwork: (lab: string, net: string) =>
    request<unknown>('DELETE', `/api/labs/${lab}/networks/${net}`),

  createLink: (lab: string, body: { a: Endpoint; b: Endpoint }) =>
    request<Link>('POST', `/api/labs/${lab}/links`, body),
  updateLink: (
    lab: string,
    link: string,
    body: { label?: string; impairment?: Impairment | null; suspended?: boolean },
  ) => request<Link>('PUT', `/api/labs/${lab}/links/${link}`, body),
  exportConfig: (lab: string, node: string) =>
    request<{ exported: string; config: string }>(
      'POST',
      `/api/labs/${lab}/nodes/${node}/config/export`,
    ),
  deleteLink: (lab: string, link: string) =>
    request<unknown>('DELETE', `/api/labs/${lab}/links/${link}`),

  createAnnotation: (lab: string, body: Partial<Annotation> & { kind: AnnotationKind; x: number; y: number }) =>
    request<Annotation>('POST', `/api/labs/${lab}/annotations`, body),
  updateAnnotation: (lab: string, ann: string, body: Partial<Annotation>) =>
    request<Annotation>('PUT', `/api/labs/${lab}/annotations/${ann}`, body),
  deleteAnnotation: (lab: string, ann: string) =>
    request<unknown>('DELETE', `/api/labs/${lab}/annotations/${ann}`),

  startCapture: (lab: string, node: string, iface: number) =>
    request<unknown>('POST', `/api/labs/${lab}/nodes/${node}/interfaces/${iface}/capture/start`),
  stopCapture: (lab: string, node: string, iface: number) =>
    request<unknown>('POST', `/api/labs/${lab}/nodes/${node}/interfaces/${iface}/capture/stop`),
  captureUrl: (lab: string, node: string, iface: number) =>
    `/api/labs/${lab}/nodes/${node}/interfaces/${iface}/capture.pcap`,

  exportUrl: (lab: string) => `/api/labs/${lab}/export`,
  import: async (file: File): Promise<Lab> => {
    const res = await fetch('/api/import', { method: 'POST', body: file })
    const text = await res.text()
    if (!res.ok) {
      let message = text
      try {
        message = JSON.parse(text).error ?? text
      } catch {
        /* raw */
      }
      throw new ApiError(res.status, message)
    }
    return JSON.parse(text)
  },
}

export function wsUrl(path: string): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws'
  return `${proto}://${location.host}${path}`
}

// Interface name from a template pattern like "Gi0/{i}" / "eth{i}" / "Gi{i+1}".
export function ifaceName(pattern: string, index: number): string {
  const m = pattern.match(/^(.*)\{i(\+(\d+))?\}(.*)$/)
  if (!m) return `${pattern}${index}`
  const offset = m[3] ? parseInt(m[3], 10) : 0
  return `${m[1]}${index + offset}${m[4]}`
}
