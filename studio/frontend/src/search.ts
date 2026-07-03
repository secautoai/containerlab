// Pure node-search helper for the canvas. Kept framework-free for unit testing.
import type { GraphNode } from "./api";

// matchNodes returns the set of node names matching a query (case-insensitive
// substring match against name and kind). An empty/whitespace query matches
// nothing (callers treat "no query" as "no filtering").
export function matchNodes(nodes: GraphNode[], query: string): Set<string> {
  const q = query.trim().toLowerCase();
  const out = new Set<string>();
  if (!q) return out;

  for (const n of nodes) {
    if (n.name.toLowerCase().includes(q) || (n.kind ?? "").toLowerCase().includes(q)) {
      out.add(n.name);
    }
  }

  return out;
}
