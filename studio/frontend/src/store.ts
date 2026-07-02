import { create } from "zustand";
import { recordPast, applyUndo, applyRedo } from "./history";
import {
  api,
  type Capabilities,
  type Graph,
  type GraphLink,
  type GraphNode,
  type KindInfo,
  type LabStatus,
  type LabSummary,
  type LintResult,
  type ValidationReport,
} from "./api";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  message: string;
}

interface StudioState {
  // static/backend data
  catalog: KindInfo[];
  capabilities?: Capabilities;
  labs: LabSummary[];

  // current lab
  graph?: Graph;
  status?: LabStatus;
  dirty: boolean;
  selectedNode?: string;
  past: Graph[];
  future: Graph[];

  // ui
  theme: "dark" | "light";
  searchQuery: string;
  toasts: Toast[];
  consoleNode?: string;
  copilotOpen: boolean;
  yamlEditorOpen: boolean;
  validation?: ValidationReport;
  validating: boolean;
  lint?: LintResult;

  // actions
  init: () => Promise<void>;
  login: (token: string) => Promise<boolean>;
  logout: () => Promise<void>;
  refreshLabs: () => Promise<void>;
  openLab: (name: string) => Promise<void>;
  createLab: (name: string) => Promise<void>;
  deleteLab: (name: string) => Promise<void>;
  saveGraph: () => Promise<void>;
  applyProposedGraph: (g: Graph, deploy?: boolean) => Promise<void>;
  adoptGraph: (g: Graph) => void;
  setGraph: (updater: (g: Graph) => Graph) => void;
  addNode: (kind: KindInfo, position: { x: number; y: number }) => void;
  updateNode: (name: string, patch: Partial<GraphNode>) => void;
  removeNode: (name: string) => void;
  addLink: (link: GraphLink) => void;
  removeLink: (index: number) => void;
  selectNode: (name?: string) => void;
  setSearch: (q: string) => void;
  undo: () => void;
  redo: () => void;
  deploy: () => Promise<void>;
  destroy: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  validate: () => Promise<void>;
  clearValidation: () => void;
  checkGraph: () => Promise<void>;
  clearLint: () => void;
  nodeAction: (node: string, action: "start" | "stop" | "restart") => Promise<void>;
  impairNode: (node: string, params: import("./api").ImpairmentParams) => Promise<void>;
  throughputTest: (from: string, to: string) => Promise<void>;
  configureLab: (protocol: "none" | "ospf" | "bgp") => Promise<void>;
  importLab: (yaml: string, name?: string) => Promise<void>;
  saveConfigs: () => Promise<void>;
  createFromTemplate: (templateId: string, name?: string) => Promise<void>;
  cloneLab: (name: string, newName: string) => Promise<void>;
  renameLab: (name: string, newName: string) => Promise<void>;
  openConsole: (node?: string) => void;
  toggleCopilot: (open?: boolean) => void;
  toggleYamlEditor: (open?: boolean) => void;
  applyYaml: (yaml: string) => Promise<void>;
  toggleTheme: () => void;
  toast: (kind: Toast["kind"], message: string) => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 1;

// getInitialTheme reads the persisted theme, guarding against non-browser
// environments (e.g. unit tests) where localStorage is unavailable.
function getInitialTheme(): "dark" | "light" {
  if (typeof localStorage === "undefined") return "dark";
  return (localStorage.getItem("clabstudio-theme") as "dark" | "light") || "dark";
}

// nextInterface returns the next free data-plane interface name for a node,
// using its kind interface pattern (e.g. eth{n}, e1-{n}).
function nextInterface(g: Graph, nodeName: string, catalog: KindInfo[]): string {
  const node = g.nodes.find((n) => n.name === nodeName);
  const kind = catalog.find((k) => k.kind === node?.kind);
  const pattern = kind?.interfacePattern || "eth{n}";

  const used = new Set<string>();
  for (const l of g.links) {
    if (l.source === nodeName && l.sourceEndpoint) used.add(l.sourceEndpoint);
    if (l.target === nodeName && l.targetEndpoint) used.add(l.targetEndpoint);
  }

  for (let i = 1; i < 256; i++) {
    const name = pattern.replace("{n}", String(i));
    if (!used.has(name)) return name;
  }
  return pattern.replace("{n}", "1");
}

// uniqueNodeName generates a unique node name based on a kind prefix.
function uniqueNodeName(g: Graph, kind: string): string {
  const base = kind.split("_").pop() || "node";
  for (let i = 1; i < 1000; i++) {
    const name = `${base}${i}`;
    if (!g.nodes.some((n) => n.name === name)) return name;
  }
  return `${base}${Date.now()}`;
}

export const useStore = create<StudioState>((set, get) => ({
  catalog: [],
  labs: [],
  dirty: false,
  past: [],
  future: [],
  theme: getInitialTheme(),
  searchQuery: "",
  toasts: [],
  copilotOpen: false,
  yamlEditorOpen: false,
  validating: false,
  lint: undefined,

  init: async () => {
    applyTheme(get().theme);
    try {
      const capabilities = await api.capabilities();
      set({ capabilities });
      // When auth is required and we're not authenticated, stop here; the login
      // screen will call init() again after a successful login.
      if (capabilities.authRequired && !capabilities.authenticated) return;
      const catalog = await api.catalog();
      set({ catalog });
    } catch (e) {
      get().toast("error", `Failed to load backend: ${(e as Error).message}`);
      return;
    }
    await get().refreshLabs();
  },

  login: async (token) => {
    try {
      await api.login(token);
      await get().init();
      return true;
    } catch (e) {
      get().toast("error", `Login failed: ${(e as Error).message}`);
      return false;
    }
  },

  logout: async () => {
    try {
      await api.logout();
    } catch {
      /* ignore */
    }
    set({ graph: undefined, labs: [], status: undefined });
    await get().init();
  },

  refreshLabs: async () => {
    try {
      const labs = await api.listLabs();
      set({ labs });
    } catch (e) {
      get().toast("error", `Failed to list labs: ${(e as Error).message}`);
    }
  },

  openLab: async (name) => {
    try {
      const graph = await api.getLab(name);
      if (!graph.nodes) graph.nodes = [];
      if (!graph.links) graph.links = [];
      set({ graph, dirty: false, selectedNode: undefined, status: undefined, past: [], future: [] });
      await get().refreshStatus();
    } catch (e) {
      get().toast("error", `Failed to open lab: ${(e as Error).message}`);
    }
  },

  createLab: async (name) => {
    try {
      const graph = await api.createLab(name);
      if (!graph.nodes) graph.nodes = [];
      if (!graph.links) graph.links = [];
      set({ graph, dirty: false, past: [], future: [] });
      await get().refreshLabs();
      get().toast("success", `Created lab "${name}"`);
    } catch (e) {
      get().toast("error", `Failed to create lab: ${(e as Error).message}`);
    }
  },

  deleteLab: async (name) => {
    try {
      await api.deleteLab(name);
      const g = get().graph;
      if (g?.name === name) set({ graph: undefined, status: undefined });
      await get().refreshLabs();
      get().toast("success", `Deleted lab "${name}"`);
    } catch (e) {
      get().toast("error", `Failed to delete lab: ${(e as Error).message}`);
    }
  },

  saveGraph: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      await api.saveLab(g);
      set({ dirty: false });
      await get().refreshLabs();
      get().toast("success", "Lab saved");
    } catch (e) {
      get().toast("error", `Failed to save: ${(e as Error).message}`);
    }
  },

  applyProposedGraph: async (g, deploy) => {
    // Ensure required arrays exist, then persist the AI-proposed topology.
    const graph: Graph = { ...g, nodes: g.nodes ?? [], links: g.links ?? [] };
    set({ graph, dirty: true, selectedNode: undefined, status: undefined, past: [], future: [] });
    await get().saveGraph();
    get().toast("success", `Applied topology "${graph.name}"`);
    if (deploy) await get().deploy();
  },

  adoptGraph: (g) => {
    // Adopt a server-side already-saved graph (e.g. a Copilot edit) into the
    // canvas without marking it dirty.
    const graph: Graph = { ...g, nodes: g.nodes ?? [], links: g.links ?? [] };
    set({ graph, dirty: false, selectedNode: undefined, past: [], future: [] });
    get().refreshLabs();
  },

  setGraph: (updater) => {
    const g = get().graph;
    if (!g) return;
    // Snapshot the current graph for undo, then apply the edit.
    set({
      past: recordPast(get().past, structuredClone(g)),
      future: [],
      graph: updater(structuredClone(g)),
      dirty: true,
    });
  },

  undo: () => {
    const g = get().graph;
    if (!g) return;
    const t = applyUndo(get().past, g, get().future);
    if (!t) return;
    set({ past: t.past, graph: t.present, future: t.future, dirty: true, selectedNode: undefined });
  },

  redo: () => {
    const g = get().graph;
    if (!g) return;
    const t = applyRedo(get().past, g, get().future);
    if (!t) return;
    set({ past: t.past, graph: t.present, future: t.future, dirty: true, selectedNode: undefined });
  },

  addNode: (kind, position) => {
    get().setGraph((g) => {
      const name = uniqueNodeName(g, kind.kind);
      const node: GraphNode = {
        name,
        kind: kind.kind,
        image: kind.defaultImage,
        icon: kind.icon,
        position,
      };
      g.nodes.push(node);
      return g;
    });
    get().toast("info", `Added ${kind.displayName}`);
  },

  updateNode: (name, patch) => {
    get().setGraph((g) => {
      const idx = g.nodes.findIndex((n) => n.name === name);
      if (idx >= 0) {
        const renamed = patch.name && patch.name !== name;
        g.nodes[idx] = { ...g.nodes[idx], ...patch };
        if (renamed) {
          for (const l of g.links) {
            if (l.source === name) l.source = patch.name!;
            if (l.target === name) l.target = patch.name!;
          }
        }
      }
      return g;
    });
    if (patch.name && patch.name !== name) set({ selectedNode: patch.name });
  },

  removeNode: (name) => {
    get().setGraph((g) => {
      g.nodes = g.nodes.filter((n) => n.name !== name);
      g.links = g.links.filter((l) => l.source !== name && l.target !== name);
      return g;
    });
    if (get().selectedNode === name) set({ selectedNode: undefined });
  },

  addLink: (link) => {
    const g = get().graph;
    if (!g) return;
    const catalog = get().catalog;
    const src = link.sourceEndpoint || nextInterface(g, link.source, catalog);
    // Add source endpoint first so the target computation sees it as used.
    const withSource: Graph = structuredClone(g);
    const dst =
      link.targetEndpoint || nextInterface(withSource, link.target, catalog);
    get().setGraph((gg) => {
      gg.links.push({ ...link, sourceEndpoint: src, targetEndpoint: dst });
      return gg;
    });
  },

  removeLink: (index) => {
    get().setGraph((g) => {
      g.links.splice(index, 1);
      return g;
    });
  },

  selectNode: (name) => set({ selectedNode: name }),
  setSearch: (q) => set({ searchQuery: q }),

  checkGraph: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      const lint = await api.lint(g);
      set({ lint });
      if (lint.ok && lint.warnings === 0) get().toast("success", "No issues found");
    } catch (e) {
      get().toast("error", `Check failed: ${(e as Error).message}`);
    }
  },

  clearLint: () => set({ lint: undefined }),

  deploy: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      // Pre-flight lint: block deployment when there are errors.
      const lint = await api.lint(g);
      if (!lint.ok) {
        set({ lint });
        get().toast("error", `Cannot deploy: ${lint.errors} error(s) in topology`);
        return;
      }
      if (get().dirty) await get().saveGraph();
      get().toast("info", `Deploying "${g.name}"…`);
      const status = await api.deploy(g.name);
      set({ status });
      await get().refreshLabs();
      get().toast("success", `Deployed "${g.name}"`);
    } catch (e) {
      get().toast("error", `Deploy failed: ${(e as Error).message}`);
    }
  },

  destroy: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      get().toast("info", `Destroying "${g.name}"…`);
      await api.destroy(g.name);
      await get().refreshStatus();
      await get().refreshLabs();
      get().toast("success", `Destroyed "${g.name}"`);
    } catch (e) {
      get().toast("error", `Destroy failed: ${(e as Error).message}`);
    }
  },

  refreshStatus: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      const status = await api.status(g.name);
      set({ status });
    } catch {
      // ignore; status is best-effort
    }
  },

  validate: async () => {
    const g = get().graph;
    if (!g) return;
    set({ validating: true });
    try {
      const validation = await api.validate(g.name);
      set({ validation });
      get().toast(validation.failed === 0 ? "success" : "error", validation.summary);
    } catch (e) {
      get().toast("error", `Validation failed: ${(e as Error).message}`);
    } finally {
      set({ validating: false });
    }
  },

  clearValidation: () => set({ validation: undefined }),

  nodeAction: async (node, action) => {
    const g = get().graph;
    if (!g) return;
    try {
      await api.nodeLifecycle(g.name, node, action);
      get().toast("success", `${action} ${node}`);
      await get().refreshStatus();
    } catch (e) {
      get().toast("error", `${action} failed: ${(e as Error).message}`);
    }
  },

  impairNode: async (node, params) => {
    const g = get().graph;
    if (!g) return;
    try {
      await api.impair(g.name, node, params);
      const cleared =
        !params.delayMs && !params.jitterMs && !params.lossPct && !params.rateKbit && !params.corruptionPct;
      get().toast("success", `${cleared ? "Cleared" : "Applied"} impairments on ${node}/${params.interface}`);
    } catch (e) {
      get().toast("error", `Impairment failed: ${(e as Error).message}`);
    }
  },

  configureLab: async (protocol) => {
    const g = get().graph;
    if (!g) return;
    try {
      if (get().dirty) await get().saveGraph();
      const res = await api.configure(g.name, protocol);
      if (res.graph.nodes == null) res.graph.nodes = [];
      if (res.graph.links == null) res.graph.links = [];
      set({ graph: res.graph, dirty: false, past: [], future: [] });
      get().toast("success", `Auto-config: ${res.plan.summary}`);
    } catch (e) {
      get().toast("error", `Auto-config failed: ${(e as Error).message}`);
    }
  },

  importLab: async (yaml, name) => {
    try {
      const g = await api.importLab(yaml, name);
      if (g.nodes == null) g.nodes = [];
      if (g.links == null) g.links = [];
      set({ graph: g, dirty: false, selectedNode: undefined, status: undefined, past: [], future: [] });
      await get().refreshLabs();
      get().toast("success", `Imported lab "${g.name}"`);
    } catch (e) {
      get().toast("error", `Import failed: ${(e as Error).message}`);
    }
  },

  saveConfigs: async () => {
    const g = get().graph;
    if (!g) return;
    try {
      await api.saveConfigs(g.name);
      get().toast("success", "Running configs saved");
    } catch (e) {
      get().toast("error", `Save configs failed: ${(e as Error).message}`);
    }
  },

  createFromTemplate: async (templateId, name) => {
    try {
      const g = await api.labFromTemplate(templateId, name);
      if (g.nodes == null) g.nodes = [];
      if (g.links == null) g.links = [];
      set({ graph: g, dirty: false, selectedNode: undefined, status: undefined, past: [], future: [] });
      await get().refreshLabs();
      get().toast("success", `Created "${g.name}" from template`);
    } catch (e) {
      get().toast("error", `Template failed: ${(e as Error).message}`);
    }
  },

  cloneLab: async (name, newName) => {
    try {
      await api.cloneLab(name, newName);
      await get().refreshLabs();
      get().toast("success", `Cloned "${name}" → "${newName}"`);
    } catch (e) {
      get().toast("error", `Clone failed: ${(e as Error).message}`);
    }
  },

  renameLab: async (name, newName) => {
    try {
      await api.renameLab(name, newName);
      const g = get().graph;
      if (g?.name === name) await get().openLab(newName);
      await get().refreshLabs();
      get().toast("success", `Renamed "${name}" → "${newName}"`);
    } catch (e) {
      get().toast("error", `Rename failed: ${(e as Error).message}`);
    }
  },

  throughputTest: async (from, to) => {
    const g = get().graph;
    if (!g) return;
    get().toast("info", `Running iperf3 ${from} → ${to}…`);
    try {
      const res = await api.iperf(g.name, from, to);
      get().toast("success", `${from} → ${to}: ${res.summary}`);
    } catch (e) {
      get().toast("error", `iperf3 failed: ${(e as Error).message}`);
    }
  },

  openConsole: (node) => set({ consoleNode: node }),
  toggleCopilot: (open) => set((s) => ({ copilotOpen: open ?? !s.copilotOpen })),
  toggleYamlEditor: (open) => set((s) => ({ yamlEditorOpen: open ?? !s.yamlEditorOpen })),

  applyYaml: async (yaml) => {
    const g = get().graph;
    if (!g) return;
    // Let the modal surface parse errors by rethrowing.
    const updated = await api.updateYaml(g.name, yaml);
    if (updated.nodes == null) updated.nodes = [];
    if (updated.links == null) updated.links = [];
    set({ graph: updated, dirty: false, selectedNode: undefined, past: [], future: [] });
    get().toast("success", "Applied YAML");
  },

  toggleTheme: () => {
    const theme = get().theme === "dark" ? "light" : "dark";
    localStorage.setItem("clabstudio-theme", theme);
    applyTheme(theme);
    set({ theme });
  },

  toast: (kind, message) => {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { id, kind, message }] }));
    setTimeout(() => get().dismissToast(id), 4500);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

function applyTheme(theme: "dark" | "light") {
  const root = document.documentElement;
  if (theme === "dark") root.classList.add("dark");
  else root.classList.remove("dark");
}

export { nextInterface, uniqueNodeName };
