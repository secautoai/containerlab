// ClabStudio REST API client and shared types (mirrors studio/model + engine).

export interface Position {
  x: number;
  y: number;
}

export interface GraphNode {
  name: string;
  kind: string;
  image?: string;
  type?: string;
  group?: string;
  mgmtIpv4?: string;
  mgmtIpv6?: string;
  startupConfig?: string;
  binds?: string[];
  env?: Record<string, string>;
  exec?: string[];
  labels?: Record<string, string>;
  position: Position;
  icon?: string;
  state?: string;
  ipv4Address?: string;
  ipv6Address?: string;
}

export interface GraphLink {
  source: string;
  sourceEndpoint?: string;
  target: string;
  targetEndpoint?: string;
  mtu?: number;
}

export interface Graph {
  name: string;
  mgmt?: { network?: string; ipv4Subnet?: string; ipv6Subnet?: string };
  nodes: GraphNode[];
  links: GraphLink[];
}

export interface KindInfo {
  kind: string;
  displayName: string;
  vendor: string;
  defaultImage?: string;
  interfacePattern?: string;
  icon: string;
  container: boolean;
  description?: string;
}

export interface LabSummary {
  name: string;
  path?: string;
  nodeCount: number;
  deployed: boolean;
  owner?: string;
  state: string;
}

export interface NodeStatus {
  name: string;
  kind: string;
  image?: string;
  state: string;
  ipv4Address?: string;
  ipv6Address?: string;
}

export interface LabStatus {
  name: string;
  deployed: boolean;
  nodes: NodeStatus[];
}

export interface Capabilities {
  runtimeAvailable: boolean;
  runtime: string;
  reason?: string;
  aiAvailable: boolean;
}

export interface ChatReply {
  reply: string;
  proposedGraph?: Graph;
  notes?: string[];
  source: string;
}

export interface ReachabilityCheck {
  from: string;
  to: string;
  target: string;
  ok: boolean;
  detail?: string;
}

export interface ValidationReport {
  lab: string;
  deployed: boolean;
  checks: ReachabilityCheck[];
  passed: number;
  failed: number;
  summary: string;
}

async function req<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`;
    try {
      const data = await res.json();
      if (data && data.error) message = data.error;
    } catch {
      // ignore parse errors
    }
    throw new Error(message);
  }

  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export const api = {
  capabilities: () => req<Capabilities>("GET", "/api/capabilities"),
  catalog: () => req<KindInfo[]>("GET", "/api/catalog"),
  listLabs: () => req<LabSummary[]>("GET", "/api/labs"),
  createLab: (name: string) => req<Graph>("POST", "/api/labs", { name }),
  getLab: (name: string) => req<Graph>("GET", `/api/labs/${encodeURIComponent(name)}`),
  saveLab: (g: Graph) => req<Graph>("PUT", `/api/labs/${encodeURIComponent(g.name)}`, g),
  deleteLab: (name: string) => req<unknown>("DELETE", `/api/labs/${encodeURIComponent(name)}`),
  status: (name: string) => req<LabStatus>("GET", `/api/labs/${encodeURIComponent(name)}/status`),
  deploy: (name: string) => req<LabStatus>("POST", `/api/labs/${encodeURIComponent(name)}/deploy`),
  destroy: (name: string, cleanup = false) =>
    req<unknown>("POST", `/api/labs/${encodeURIComponent(name)}/destroy`, { cleanup }),
  exec: (name: string, node: string, cmd: string) =>
    req<{ stdout: string; stderr: string; returnCode: number }>(
      "POST",
      `/api/labs/${encodeURIComponent(name)}/nodes/${encodeURIComponent(node)}/exec`,
      { cmd },
    ),
  yamlURL: (name: string) => `/api/labs/${encodeURIComponent(name)}/yaml`,
  aiChat: (message: string, lab?: string) =>
    req<ChatReply>("POST", "/api/ai/chat", { message, lab }),
  validate: (name: string) =>
    req<ValidationReport>("POST", `/api/labs/${encodeURIComponent(name)}/validate`),
  nodeLifecycle: (name: string, node: string, action: "start" | "stop" | "restart") =>
    req<unknown>(
      "POST",
      `/api/labs/${encodeURIComponent(name)}/nodes/${encodeURIComponent(node)}/lifecycle`,
      { action },
    ),
  impair: (name: string, node: string, params: ImpairmentParams) =>
    req<unknown>(
      "POST",
      `/api/labs/${encodeURIComponent(name)}/nodes/${encodeURIComponent(node)}/impair`,
      params,
    ),
  configure: (name: string, protocol: "none" | "ospf" | "bgp") =>
    req<{ graph: Graph; plan: { summary: string } }>(
      "POST",
      `/api/labs/${encodeURIComponent(name)}/configure`,
      { protocol },
    ),
  importLab: (yaml: string, name?: string) => req<Graph>("POST", "/api/labs/import", { yaml, name }),
  saveConfigs: (name: string) =>
    req<unknown>("POST", `/api/labs/${encodeURIComponent(name)}/save`),
};

export interface ImpairmentParams {
  interface: string;
  delayMs?: number;
  jitterMs?: number;
  lossPct?: number;
  rateKbit?: number;
  corruptionPct?: number;
}
