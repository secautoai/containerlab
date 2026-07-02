// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import "testing"

func TestAutoLayoutAssignsPositions(t *testing.T) {
	g := &Graph{
		Name: "t",
		Nodes: []*Node{
			{Name: "a", Kind: "linux"},
			{Name: "b", Kind: "linux"},
			{Name: "c", Kind: "linux"},
			{Name: "d", Kind: "linux"},
		},
	}

	AutoLayout(g)

	seen := map[string]bool{}

	for _, n := range g.Nodes {
		if n.Position.X == 0 && n.Position.Y == 0 {
			t.Errorf("node %s did not get a position", n.Name)
		}

		key := formatPos(n.Position)
		if seen[key] {
			t.Errorf("duplicate position for node %s: %s", n.Name, key)
		}

		seen[key] = true
	}
}

func TestAutoLayoutPreservesExisting(t *testing.T) {
	g := &Graph{
		Name: "t",
		Nodes: []*Node{
			{Name: "a", Kind: "linux", Position: Position{X: 500, Y: 500}},
			{Name: "b", Kind: "linux"},
		},
	}

	AutoLayout(g)

	if g.Nodes[0].Position.X != 500 || g.Nodes[0].Position.Y != 500 {
		t.Errorf("existing position was overwritten: %+v", g.Nodes[0].Position)
	}

	if g.Nodes[1].Position.X == 0 && g.Nodes[1].Position.Y == 0 {
		t.Errorf("node b did not get a position")
	}
}

func formatPos(p Position) string {
	return string(rune(int(p.X))) + ":" + string(rune(int(p.Y)))
}
