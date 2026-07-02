// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/srl-labs/containerlab/studio/model"
)

// EditResult is the outcome of a conversational topology edit.
type EditResult struct {
	Changed bool   `json:"changed"`
	Message string `json:"message"`
}

var shapeWords = []string{"ring", "mesh", "star", "leaf", "spine", "clos", "fabric", "topology"}

// IsEditIntent reports whether a message is a conversational edit of an existing
// topology (add/connect/remove), as opposed to a request to generate a new one.
func IsEditIntent(g *model.Graph, msg string) bool {
	if g == nil || len(g.Nodes) == 0 {
		return false
	}

	p := strings.ToLower(msg)

	// Shape words imply generating a fresh topology, not editing.
	for _, w := range shapeWords {
		if strings.Contains(p, w) {
			return false
		}
	}

	// "add" is checked first so "add a host connected to r1" is an add intent.
	switch {
	case strings.Contains(p, "add"):
		return true
	case strings.Contains(p, "remove") || strings.Contains(p, "delete"):
		return len(mentionedNodes(g, p)) > 0
	case strings.Contains(p, "connect") || strings.Contains(p, "link"):
		return len(mentionedNodes(g, p)) >= 2
	default:
		return false
	}
}

// EditGraph applies a natural-language edit to the graph in place and returns
// the result. It handles: remove node(s), connect nodes, and add node(s).
func EditGraph(g *model.Graph, msg string) *EditResult {
	p := strings.ToLower(msg)

	// "add" takes priority so phrases like "add a host connected to r1" are
	// treated as an add (with attachment) rather than a bare connect.
	switch {
	case strings.Contains(p, "add"):
		return addNodes(g, p)
	case strings.Contains(p, "remove") || strings.Contains(p, "delete"):
		return removeNodes(g, p)
	case strings.Contains(p, "connect") || strings.Contains(p, "link"):
		return connectNodes(g, p)
	default:
		return &EditResult{Changed: false, Message: "I couldn't understand that edit."}
	}
}

// mentionedNodes returns existing node names referenced in the (lowercased) msg.
func mentionedNodes(g *model.Graph, p string) []string {
	var out []string

	for _, n := range g.Nodes {
		re := regexp.MustCompile(`\b` + regexp.QuoteMeta(strings.ToLower(n.Name)) + `\b`)
		if re.MatchString(p) {
			out = append(out, n.Name)
		}
	}

	// Order by position of first occurrence for predictable connect semantics.
	sort.SliceStable(out, func(i, j int) bool {
		return strings.Index(p, strings.ToLower(out[i])) < strings.Index(p, strings.ToLower(out[j]))
	})

	return out
}

func removeNodes(g *model.Graph, p string) *EditResult {
	targets := mentionedNodes(g, p)
	if len(targets) == 0 {
		return &EditResult{Changed: false, Message: "No matching node to remove."}
	}

	remove := map[string]bool{}
	for _, t := range targets {
		remove[t] = true
	}

	nodes := g.Nodes[:0]
	for _, n := range g.Nodes {
		if !remove[n.Name] {
			nodes = append(nodes, n)
		}
	}

	g.Nodes = nodes

	links := g.Links[:0]
	for _, l := range g.Links {
		if !remove[l.Source] && !remove[l.Target] {
			links = append(links, l)
		}
	}

	g.Links = links

	return &EditResult{Changed: true, Message: "Removed " + strings.Join(targets, ", ") + "."}
}

func connectNodes(g *model.Graph, p string) *EditResult {
	nodes := mentionedNodes(g, p)
	if len(nodes) < 2 {
		return &EditResult{Changed: false, Message: "Name two existing nodes to connect."}
	}

	a, b := nodes[0], nodes[1]
	addGraphLink(g, a, b)

	return &EditResult{Changed: true, Message: fmt.Sprintf("Connected %s and %s.", a, b)}
}

var countKindRe = regexp.MustCompile(`add\s+(\d+)?\s*(?:more\s+)?([a-z_]+)?`)

func addNodes(g *model.Graph, p string) *EditResult {
	kind := detectKind(p)

	// Prefer an explicit "host"/"client" hint for naming even when kind is linux.
	isHost := strings.Contains(p, "host") || strings.Contains(p, "client") ||
		strings.Contains(p, "pc") || strings.Contains(p, "server")

	count := 1

	if m := countKindRe.FindStringSubmatch(p); len(m) >= 2 && m[1] != "" {
		if n, err := strconv.Atoi(m[1]); err == nil && n > 0 {
			count = clampCount(n)
		}
	}

	// Optional "connect(ed) to <node>" attaches each new node to an existing one.
	attach := ""

	if strings.Contains(p, "connect") || strings.Contains(p, "attach") || strings.Contains(p, " to ") {
		for _, n := range mentionedNodes(g, p) {
			attach = n
			break
		}
	}

	prefix := "node"

	switch {
	case isHost:
		prefix = "host"
	case kind == "linux":
		prefix = "host"
	default:
		prefix = "r"
	}

	added := make([]string, 0, count)

	for i := 0; i < count; i++ {
		name := uniqueName(g, prefix)
		node := &model.Node{
			Name:     name,
			Kind:     kind,
			Image:    model.DefaultImageForKind(kind),
			Position: model.Position{X: 140 + float64(len(g.Nodes)%6)*180, Y: 120 + float64(len(g.Nodes)/6)*160},
		}
		g.Nodes = append(g.Nodes, node)
		added = append(added, name)

		if attach != "" {
			addGraphLink(g, name, attach)
		}
	}

	msg := fmt.Sprintf("Added %s (%s)", strings.Join(added, ", "), displayKind(kind))
	if attach != "" {
		msg += " connected to " + attach
	}

	return &EditResult{Changed: true, Message: msg + "."}
}

// uniqueName returns a unique node name with the given prefix.
func uniqueName(g *model.Graph, prefix string) string {
	for i := 1; i < 10000; i++ {
		name := fmt.Sprintf("%s%d", prefix, i)
		if !nodeExists(g, name) {
			return name
		}
	}

	return fmt.Sprintf("%s%d", prefix, len(g.Nodes)+1)
}

func nodeExists(g *model.Graph, name string) bool {
	for _, n := range g.Nodes {
		if n.Name == name {
			return true
		}
	}

	return false
}

// addGraphLink appends a link between two nodes using the next free interface
// name on each (respecting kind interface patterns).
func addGraphLink(g *model.Graph, a, b string) {
	g.Links = append(g.Links, &model.Link{
		Source:         a,
		SourceEndpoint: nextGraphIface(g, a),
		Target:         b,
		TargetEndpoint: nextGraphIface(g, b),
	})
}

// nextGraphIface computes the next free interface name for a node in the graph.
func nextGraphIface(g *model.Graph, node string) string {
	var kind string

	for _, n := range g.Nodes {
		if n.Name == node {
			kind = n.Kind
			break
		}
	}

	pattern := model.InterfacePatternForKind(kind)

	used := map[string]bool{}

	for _, l := range g.Links {
		if l.Source == node && l.SourceEndpoint != "" {
			used[l.SourceEndpoint] = true
		}

		if l.Target == node && l.TargetEndpoint != "" {
			used[l.TargetEndpoint] = true
		}
	}

	for i := 1; i < 256; i++ {
		name := strings.Replace(pattern, "{n}", strconv.Itoa(i), 1)
		if !used[name] {
			return name
		}
	}

	return strings.Replace(pattern, "{n}", "1", 1)
}
