// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import (
	"os"
	"path/filepath"
	"sort"
	"testing"
)

// nodeSet returns a comparable representation of nodes keyed by name with
// effective kind/image so semantic equivalence can be asserted across a
// round-trip that may restructure inheritance (groups/kinds) into explicit
// per-node fields.
func nodeSet(g *Graph) map[string]struct{ Kind, Image, Group string } {
	out := map[string]struct{ Kind, Image, Group string }{}
	for _, n := range g.Nodes {
		out[n.Name] = struct{ Kind, Image, Group string }{n.Kind, n.Image, n.Group}
	}

	return out
}

// linkSet returns a set of canonicalized links (endpoint order independent).
func linkSet(g *Graph) map[string]struct{} {
	out := map[string]struct{}{}

	for _, l := range g.Links {
		a := l.Source + ":" + l.SourceEndpoint
		b := l.Target + ":" + l.TargetEndpoint

		pair := []string{a, b}
		sort.Strings(pair)
		out[pair[0]+"|"+pair[1]] = struct{}{}
	}

	return out
}

func TestRoundTripLabExamples(t *testing.T) {
	examples := []string{
		"../../lab-examples/frr01/frr01.clab.yml",
		"../../lab-examples/srl01/srl01.clab.yml",
	}

	for _, path := range examples {
		path := path
		t.Run(filepath.Base(path), func(t *testing.T) {
			data, err := os.ReadFile(path)
			if err != nil {
				t.Fatalf("read example: %v", err)
			}

			g1, err := ClabYAMLToGraph(data)
			if err != nil {
				t.Fatalf("parse original: %v", err)
			}

			out, err := GraphToClabYAML(g1)
			if err != nil {
				t.Fatalf("serialize: %v", err)
			}

			g2, err := ClabYAMLToGraph(out)
			if err != nil {
				t.Fatalf("parse serialized: %v", err)
			}

			if g1.Name != g2.Name {
				t.Errorf("name mismatch: %q vs %q", g1.Name, g2.Name)
			}

			ns1, ns2 := nodeSet(g1), nodeSet(g2)
			if len(ns1) != len(ns2) {
				t.Fatalf("node count mismatch: %d vs %d", len(ns1), len(ns2))
			}

			for name, v1 := range ns1 {
				v2, ok := ns2[name]
				if !ok {
					t.Errorf("node %q lost in round-trip", name)
					continue
				}
				// kind/image are the semantically important resolved values
				if v1.Kind != v2.Kind {
					t.Errorf("node %q kind mismatch: %q vs %q", name, v1.Kind, v2.Kind)
				}
				if v1.Image != v2.Image {
					t.Errorf("node %q image mismatch: %q vs %q", name, v1.Image, v2.Image)
				}
			}

			ls1, ls2 := linkSet(g1), linkSet(g2)
			if len(ls1) != len(ls2) {
				t.Fatalf("link count mismatch: %d vs %d", len(ls1), len(ls2))
			}

			for k := range ls1 {
				if _, ok := ls2[k]; !ok {
					t.Errorf("link %q lost in round-trip", k)
				}
			}
		})
	}
}

func TestPositionPersistedViaLabels(t *testing.T) {
	g := &Graph{
		Name: "postest",
		Nodes: []*Node{
			{Name: "n1", Kind: "linux", Image: "alpine", Position: Position{X: 120.5, Y: -40}},
		},
		Links: []*Link{},
	}

	out, err := GraphToClabYAML(g)
	if err != nil {
		t.Fatalf("serialize: %v", err)
	}

	g2, err := ClabYAMLToGraph(out)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if len(g2.Nodes) != 1 {
		t.Fatalf("expected 1 node, got %d", len(g2.Nodes))
	}

	got := g2.Nodes[0].Position
	if got.X != 120.5 || got.Y != -40 {
		t.Errorf("position not preserved: got %+v", got)
	}
}

func TestGraphToYAMLRequiresName(t *testing.T) {
	_, err := GraphToClabYAML(&Graph{})
	if err == nil {
		t.Fatal("expected error for empty name")
	}
}

func TestLinkEndpointParsing(t *testing.T) {
	data := []byte(`name: l
topology:
  nodes:
    a: {kind: linux, image: alpine}
    b: {kind: linux, image: alpine}
  links:
    - endpoints: ["a:eth1", "b:eth2"]
`)

	g, err := ClabYAMLToGraph(data)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if len(g.Links) != 1 {
		t.Fatalf("expected 1 link, got %d", len(g.Links))
	}

	l := g.Links[0]
	if l.Source != "a" || l.SourceEndpoint != "eth1" || l.Target != "b" || l.TargetEndpoint != "eth2" {
		t.Errorf("unexpected link parse: %+v", l)
	}
}
