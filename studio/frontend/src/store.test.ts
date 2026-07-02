import { describe, it, expect } from "vitest";
import { nextInterface, uniqueNodeName } from "./store";
import type { Graph, KindInfo } from "./api";

const catalog: KindInfo[] = [
  { kind: "linux", displayName: "Linux", vendor: "Generic", icon: "server", container: true, interfacePattern: "eth{n}" },
  { kind: "nokia_srlinux", displayName: "SR Linux", vendor: "Nokia", icon: "router", container: true, interfacePattern: "e1-{n}" },
];

function graph(): Graph {
  return {
    name: "t",
    nodes: [
      { name: "r1", kind: "nokia_srlinux", position: { x: 0, y: 0 } },
      { name: "h1", kind: "linux", position: { x: 0, y: 0 } },
    ],
    links: [{ source: "r1", sourceEndpoint: "e1-1", target: "h1", targetEndpoint: "eth1" }],
  };
}

describe("nextInterface", () => {
  it("uses the kind interface pattern and skips used interfaces", () => {
    const g = graph();
    // r1 already uses e1-1 -> next should be e1-2
    expect(nextInterface(g, "r1", catalog)).toBe("e1-2");
    // h1 already uses eth1 -> next should be eth2
    expect(nextInterface(g, "h1", catalog)).toBe("eth2");
  });

  it("defaults to eth{n} for unknown kinds", () => {
    const g: Graph = { name: "t", nodes: [{ name: "x", kind: "mystery", position: { x: 0, y: 0 } }], links: [] };
    expect(nextInterface(g, "x", catalog)).toBe("eth1");
  });
});

describe("uniqueNodeName", () => {
  it("derives a name from the kind suffix and avoids collisions", () => {
    const g = graph();
    // suffix of nokia_srlinux is "srlinux"
    expect(uniqueNodeName(g, "nokia_srlinux")).toBe("srlinux1");
  });

  it("increments when the base name is taken", () => {
    const g: Graph = {
      name: "t",
      nodes: [{ name: "linux1", kind: "linux", position: { x: 0, y: 0 } }],
      links: [],
    };
    expect(uniqueNodeName(g, "linux")).toBe("linux2");
  });
});
