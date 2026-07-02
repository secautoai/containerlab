// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"fmt"
	"math"
	"regexp"
	"strconv"
	"strings"

	"github.com/srl-labs/containerlab/studio/model"
)

// GenResult is the output of the offline topology generator.
type GenResult struct {
	Graph   *model.Graph `json:"graph"`
	Summary string       `json:"summary"`
	Notes   []string     `json:"notes,omitempty"`
}

// Topology shapes understood by the generator.
type topoShape string

const (
	shapeLinear    topoShape = "linear"
	shapeRing      topoShape = "ring"
	shapeStar      topoShape = "star"
	shapeMesh      topoShape = "mesh"
	shapeLeafSpine topoShape = "leaf-spine"
	shapeTriangle  topoShape = "triangle"
)

var numRe = regexp.MustCompile(`(\d+)`)

// ifaceCounter tracks the next data interface index per node.
type ifaceCounter struct {
	counts map[string]int
}

func newIfaceCounter() *ifaceCounter { return &ifaceCounter{counts: map[string]int{}} }

func (c *ifaceCounter) next(node, kind string) string {
	c.counts[node]++
	pattern := model.InterfacePatternForKind(kind)

	return strings.Replace(pattern, "{n}", strconv.Itoa(c.counts[node]), 1)
}

// Generate turns a plain-English prompt into a lab topology graph using
// deterministic heuristics. It always returns a usable topology.
func Generate(prompt, labName string) (*GenResult, error) {
	p := strings.ToLower(prompt)

	kind := detectKind(p)
	shape := detectShape(p)
	count := detectCount(p, shape)

	if labName == "" {
		labName = suggestLabName(shape, kind)
	}

	g := &model.Graph{Name: labName, Nodes: []*model.Node{}, Links: []*model.Link{}}
	ic := newIfaceCounter()

	var notes []string

	switch shape {
	case shapeLeafSpine:
		leaves, spines := detectLeafSpine(p)
		buildLeafSpine(g, ic, kind, leaves, spines)
		notes = append(notes, fmt.Sprintf("%d spines x %d leaves, each leaf uplinked to every spine", spines, leaves))
	case shapeMesh:
		buildLinearOrCircular(g, ic, kind, count, shapeMesh)
		notes = append(notes, "full mesh: every node connected to every other node")
	case shapeRing:
		buildLinearOrCircular(g, ic, kind, count, shapeRing)
	case shapeStar:
		buildStar(g, ic, kind, count)
		notes = append(notes, "hub-and-spoke: node1 is the hub")
	case shapeTriangle:
		buildLinearOrCircular(g, ic, kind, 3, shapeRing)
	default:
		buildLinearOrCircular(g, ic, kind, count, shapeLinear)
	}

	// Optionally attach linux hosts if the prompt asks for hosts/clients/PCs.
	if wantsHosts(p) && shape != shapeLeafSpine {
		attachHosts(g, ic)
		notes = append(notes, "attached a Linux host to each router")
	}

	proto := detectProtocols(p)
	if len(proto) > 0 {
		notes = append(notes, "requested protocols: "+strings.Join(proto, ", ")+
			" (generate configs from the node drawer or a follow-up request)")
	}

	res := &GenResult{
		Graph:   g,
		Summary: summarize(shape, kind, g, proto),
		Notes:   notes,
	}

	return res, nil
}

func detectKind(p string) string {
	switch {
	case strings.Contains(p, "srlinux") || strings.Contains(p, "sr linux") || strings.Contains(p, "nokia"):
		return "nokia_srlinux"
	case strings.Contains(p, "arista") || strings.Contains(p, "ceos") || strings.Contains(p, "eos"):
		return "arista_ceos"
	case strings.Contains(p, "juniper") || strings.Contains(p, "crpd") || strings.Contains(p, "junos"):
		return "juniper_crpd"
	case strings.Contains(p, "sonic"):
		return "sonic-vs"
	case strings.Contains(p, "cumulus") || strings.Contains(p, "cvx"):
		return "cvx"
	case strings.Contains(p, "frr") || strings.Contains(p, "freerouter") || strings.Contains(p, "free range"):
		return "linux" // FRR runs in a linux container
	case strings.Contains(p, "linux") || strings.Contains(p, "alpine") || strings.Contains(p, "ubuntu"):
		return "linux"
	default:
		return "nokia_srlinux"
	}
}

func detectShape(p string) topoShape {
	switch {
	case strings.Contains(p, "leaf") || strings.Contains(p, "spine") || strings.Contains(p, "clos") || strings.Contains(p, "fabric"):
		return shapeLeafSpine
	case strings.Contains(p, "ring"):
		return shapeRing
	case strings.Contains(p, "star") || strings.Contains(p, "hub") || strings.Contains(p, "spoke"):
		return shapeStar
	case strings.Contains(p, "mesh"):
		return shapeMesh
	case strings.Contains(p, "triangle"):
		return shapeTriangle
	case strings.Contains(p, "chain") || strings.Contains(p, "linear") || strings.Contains(p, "line") || strings.Contains(p, "back-to-back") || strings.Contains(p, "point-to-point"):
		return shapeLinear
	default:
		return shapeLinear
	}
}

func detectCount(p string, shape topoShape) int {
	if shape == shapeTriangle {
		return 3
	}

	// Prefer a number that appears near a node keyword.
	kw := regexp.MustCompile(`(\d+)\s*(routers?|nodes?|switches?|devices?|hosts?)`)
	if m := kw.FindStringSubmatch(p); len(m) == 3 {
		if n, err := strconv.Atoi(m[1]); err == nil {
			return clampCount(n)
		}
	}

	if m := numRe.FindString(p); m != "" {
		if n, err := strconv.Atoi(m); err == nil {
			return clampCount(n)
		}
	}

	// sensible defaults per shape
	switch shape {
	case shapeStar:
		return 4
	case shapeRing, shapeMesh:
		return 4
	default:
		return 2
	}
}

func detectLeafSpine(p string) (leaves, spines int) {
	leaves, spines = 2, 2

	if m := regexp.MustCompile(`(\d+)\s*leaf`).FindStringSubmatch(p); len(m) == 2 {
		if n, err := strconv.Atoi(m[1]); err == nil {
			leaves = clampCount(n)
		}
	}

	if m := regexp.MustCompile(`(\d+)\s*spine`).FindStringSubmatch(p); len(m) == 2 {
		if n, err := strconv.Atoi(m[1]); err == nil {
			spines = clampCount(n)
		}
	}

	return leaves, spines
}

func clampCount(n int) int {
	if n < 1 {
		return 1
	}

	if n > 24 {
		return 24
	}

	return n
}

func detectProtocols(p string) []string {
	var out []string

	for _, proto := range []string{"ospf", "bgp", "isis", "mpls", "evpn", "vxlan", "rip"} {
		if strings.Contains(p, proto) {
			out = append(out, strings.ToUpper(proto))
		}
	}

	return out
}

func wantsHosts(p string) bool {
	return strings.Contains(p, "host") || strings.Contains(p, "client") ||
		strings.Contains(p, "pc") || strings.Contains(p, "server") ||
		strings.Contains(p, "endpoint")
}

func nodeName(kind string, i int) string {
	prefix := "node"

	switch kind {
	case "nokia_srlinux", "arista_ceos", "juniper_crpd", "cvx", "sonic-vs":
		prefix = "r"
	case "linux":
		prefix = "r" // routers by default; hosts get their own prefix
	}

	return fmt.Sprintf("%s%d", prefix, i)
}

func imageFor(kind string) string {
	return model.DefaultImageForKind(kind)
}

func addNode(g *model.Graph, name, kind string, x, y float64) *model.Node {
	n := &model.Node{
		Name:     name,
		Kind:     kind,
		Image:    imageFor(kind),
		Position: model.Position{X: x, Y: y},
	}
	g.Nodes = append(g.Nodes, n)

	return n
}

func addLink(g *model.Graph, ic *ifaceCounter, a, aKind, b, bKind string) {
	g.Links = append(g.Links, &model.Link{
		Source:         a,
		SourceEndpoint: ic.next(a, aKind),
		Target:         b,
		TargetEndpoint: ic.next(b, bKind),
	})
}

func buildLinearOrCircular(g *model.Graph, ic *ifaceCounter, kind string, count int, shape topoShape) {
	count = clampCount(count)

	const (
		radius = 220.0
		cx     = 380.0
		cy     = 300.0
		gap    = 200.0
	)

	for i := 1; i <= count; i++ {
		var x, y float64

		if shape == shapeLinear {
			x = 120 + float64(i-1)*gap
			y = 240
		} else {
			ang := 2 * math.Pi * float64(i-1) / float64(count)
			x = cx + radius*math.Cos(ang)
			y = cy + radius*math.Sin(ang)
		}

		addNode(g, nodeName(kind, i), kind, x, y)
	}

	switch shape {
	case shapeMesh:
		for i := 1; i <= count; i++ {
			for j := i + 1; j <= count; j++ {
				addLink(g, ic, nodeName(kind, i), kind, nodeName(kind, j), kind)
			}
		}
	case shapeRing:
		for i := 1; i <= count; i++ {
			next := i%count + 1
			if next == i {
				continue
			}

			addLink(g, ic, nodeName(kind, i), kind, nodeName(kind, next), kind)
		}
	default: // linear
		for i := 1; i < count; i++ {
			addLink(g, ic, nodeName(kind, i), kind, nodeName(kind, i+1), kind)
		}
	}
}

func buildStar(g *model.Graph, ic *ifaceCounter, kind string, count int) {
	count = clampCount(count)
	if count < 2 {
		count = 2
	}

	addNode(g, nodeName(kind, 1), kind, 380, 280) // hub

	spokes := count - 1
	for i := 2; i <= count; i++ {
		ang := 2 * math.Pi * float64(i-2) / float64(spokes)
		x := 380 + 220*math.Cos(ang)
		y := 280 + 220*math.Sin(ang)
		addNode(g, nodeName(kind, i), kind, x, y)
		addLink(g, ic, nodeName(kind, 1), kind, nodeName(kind, i), kind)
	}
}

func buildLeafSpine(g *model.Graph, ic *ifaceCounter, kind string, leaves, spines int) {
	leaves = clampCount(leaves)
	spines = clampCount(spines)

	spineNames := make([]string, 0, spines)
	for s := 1; s <= spines; s++ {
		name := fmt.Sprintf("spine%d", s)
		x := 240 + float64(s-1)*240
		addNode(g, name, kind, x, 120)
		spineNames = append(spineNames, name)
	}

	for l := 1; l <= leaves; l++ {
		name := fmt.Sprintf("leaf%d", l)
		x := 160 + float64(l-1)*200
		addNode(g, name, kind, x, 400)

		for _, sp := range spineNames {
			addLink(g, ic, name, kind, sp, kind)
		}
	}
}

// attachHosts adds one Linux host per router-like node.
func attachHosts(g *model.Graph, ic *ifaceCounter) {
	routers := make([]*model.Node, 0, len(g.Nodes))
	for _, n := range g.Nodes {
		routers = append(routers, n)
	}

	h := 0

	for _, r := range routers {
		h++
		name := fmt.Sprintf("host%d", h)
		host := addNode(g, name, "linux", r.Position.X, r.Position.Y+160)
		addLink(g, ic, host.Name, "linux", r.Name, r.Kind)
	}
}

func suggestLabName(shape topoShape, kind string) string {
	k := strings.ReplaceAll(kind, "_", "-")

	return fmt.Sprintf("%s-%s", k, shape)
}

func summarize(shape topoShape, kind string, g *model.Graph, proto []string) string {
	var b strings.Builder

	fmt.Fprintf(&b, "Generated a %s topology with %d nodes and %d links using %s.",
		shape, len(g.Nodes), len(g.Links), displayKind(kind))

	if len(proto) > 0 {
		fmt.Fprintf(&b, " Protocols to configure: %s.", strings.Join(proto, ", "))
	}

	b.WriteString(" Review it on the canvas, then Apply and Deploy.")

	return b.String()
}

func displayKind(kind string) string {
	for _, k := range model.Catalog() {
		if k.Kind == kind {
			return k.DisplayName
		}
	}

	return kind
}
