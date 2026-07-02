// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"testing"

	"github.com/srl-labs/containerlab/studio/model"
)

func editGraph() *model.Graph {
	return &model.Graph{
		Name: "e",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "nokia_srlinux", Image: "srl"},
			{Name: "r2", Kind: "nokia_srlinux", Image: "srl"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "e1-1", Target: "r2", TargetEndpoint: "e1-1"},
		},
	}
}

func TestIsEditIntent(t *testing.T) {
	g := editGraph()

	if !IsEditIntent(g, "add a linux host") {
		t.Error("expected add to be an edit intent")
	}

	if !IsEditIntent(g, "connect r1 to r2") {
		t.Error("expected connect to be an edit intent")
	}

	if !IsEditIntent(g, "remove r2") {
		t.Error("expected remove to be an edit intent")
	}

	// shape words => generate, not edit
	if IsEditIntent(g, "build a 4 node ring") {
		t.Error("ring should not be an edit intent")
	}

	// no graph => not an edit
	if IsEditIntent(&model.Graph{}, "add a node") {
		t.Error("empty graph should not be an edit intent")
	}
}

func TestEditAddNode(t *testing.T) {
	g := editGraph()

	res := EditGraph(g, "add a linux host connected to r1")
	if !res.Changed {
		t.Fatalf("expected change: %s", res.Message)
	}

	if len(g.Nodes) != 3 {
		t.Fatalf("expected 3 nodes, got %d", len(g.Nodes))
	}

	// new host should be linked to r1 (2 links now)
	if len(g.Links) != 2 {
		t.Fatalf("expected 2 links, got %d", len(g.Links))
	}

	host := g.Nodes[2]
	if host.Kind != "linux" {
		t.Errorf("expected linux host, got %s", host.Kind)
	}
}

func TestEditAddMultiple(t *testing.T) {
	g := editGraph()

	res := EditGraph(g, "add 3 srlinux routers")
	if !res.Changed || len(g.Nodes) != 5 {
		t.Fatalf("expected 5 nodes, got %d (%s)", len(g.Nodes), res.Message)
	}
}

func TestEditConnect(t *testing.T) {
	g := editGraph()
	g.Nodes = append(g.Nodes, &model.Node{Name: "r3", Kind: "nokia_srlinux", Image: "srl"})

	res := EditGraph(g, "connect r1 to r3")
	if !res.Changed {
		t.Fatalf("expected change: %s", res.Message)
	}

	if len(g.Links) != 2 {
		t.Fatalf("expected 2 links, got %d", len(g.Links))
	}

	// r1 should now have e1-2 (e1-1 already used)
	last := g.Links[len(g.Links)-1]
	if last.Source != "r1" || last.Target != "r3" {
		t.Errorf("unexpected link: %+v", last)
	}

	if last.SourceEndpoint != "e1-2" {
		t.Errorf("expected r1 next iface e1-2, got %s", last.SourceEndpoint)
	}
}

func TestEditRemove(t *testing.T) {
	g := editGraph()

	res := EditGraph(g, "remove r2")
	if !res.Changed {
		t.Fatalf("expected change: %s", res.Message)
	}

	if len(g.Nodes) != 1 || g.Nodes[0].Name != "r1" {
		t.Fatalf("expected only r1 left, got %+v", g.Nodes)
	}

	// link referencing r2 should be gone
	if len(g.Links) != 0 {
		t.Fatalf("expected 0 links after removing r2, got %d", len(g.Links))
	}
}
