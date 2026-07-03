import { describe, it, expect } from "vitest";
import { matchNodes } from "./search";
import type { GraphNode } from "./api";

const nodes: GraphNode[] = [
  { name: "r1", kind: "nokia_srlinux", position: { x: 0, y: 0 } },
  { name: "r2", kind: "nokia_srlinux", position: { x: 0, y: 0 } },
  { name: "host1", kind: "linux", position: { x: 0, y: 0 } },
];

describe("matchNodes", () => {
  it("matches by name substring", () => {
    expect(matchNodes(nodes, "r1")).toEqual(new Set(["r1"]));
  });

  it("matches by kind", () => {
    expect(matchNodes(nodes, "srlinux")).toEqual(new Set(["r1", "r2"]));
    expect(matchNodes(nodes, "linux")).toEqual(new Set(["r1", "r2", "host1"]));
  });

  it("is case-insensitive", () => {
    expect(matchNodes(nodes, "HOST")).toEqual(new Set(["host1"]));
  });

  it("empty query matches nothing", () => {
    expect(matchNodes(nodes, "   ")).toEqual(new Set());
  });

  it("no match returns empty set", () => {
    expect(matchNodes(nodes, "zzz")).toEqual(new Set());
  });
});
