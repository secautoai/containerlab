// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"strings"
	"testing"
)

func TestGenerateLinear(t *testing.T) {
	res, err := Generate("build a 3 node linear srlinux lab", "")
	if err != nil {
		t.Fatalf("generate: %v", err)
	}

	if len(res.Graph.Nodes) != 3 {
		t.Fatalf("expected 3 nodes, got %d", len(res.Graph.Nodes))
	}

	if len(res.Graph.Links) != 2 {
		t.Fatalf("expected 2 links (linear), got %d", len(res.Graph.Links))
	}

	for _, n := range res.Graph.Nodes {
		if n.Kind != "nokia_srlinux" {
			t.Errorf("expected nokia_srlinux, got %s", n.Kind)
		}
	}
}

func TestGenerateRing(t *testing.T) {
	res, _ := Generate("4 router ring topology with frr", "")
	if len(res.Graph.Nodes) != 4 {
		t.Fatalf("expected 4 nodes, got %d", len(res.Graph.Nodes))
	}
	// ring => same number of links as nodes
	if len(res.Graph.Links) != 4 {
		t.Fatalf("expected 4 links (ring), got %d", len(res.Graph.Links))
	}

	if res.Graph.Nodes[0].Kind != "linux" {
		t.Errorf("frr should map to linux kind, got %s", res.Graph.Nodes[0].Kind)
	}
}

func TestGenerateMesh(t *testing.T) {
	res, _ := Generate("full mesh of 4 nodes", "")
	// full mesh of 4 => 6 links
	if len(res.Graph.Links) != 6 {
		t.Fatalf("expected 6 links (mesh of 4), got %d", len(res.Graph.Links))
	}
}

func TestGenerateStar(t *testing.T) {
	res, _ := Generate("star topology with 5 nodes hub and spoke", "")
	if len(res.Graph.Nodes) != 5 {
		t.Fatalf("expected 5 nodes, got %d", len(res.Graph.Nodes))
	}
	// star of 5 => 4 spokes
	if len(res.Graph.Links) != 4 {
		t.Fatalf("expected 4 links (star), got %d", len(res.Graph.Links))
	}
}

func TestGenerateLeafSpine(t *testing.T) {
	res, _ := Generate("clos fabric with 3 leaf and 2 spine arista", "")

	nodes := len(res.Graph.Nodes)
	if nodes != 5 {
		t.Fatalf("expected 5 nodes (3 leaf + 2 spine), got %d", nodes)
	}
	// each leaf to each spine => 3*2 = 6 links
	if len(res.Graph.Links) != 6 {
		t.Fatalf("expected 6 links (leaf-spine), got %d", len(res.Graph.Links))
	}

	if res.Graph.Nodes[0].Kind != "arista_ceos" {
		t.Errorf("expected arista_ceos, got %s", res.Graph.Nodes[0].Kind)
	}
}

func TestGenerateTriangle(t *testing.T) {
	res, _ := Generate("ospf triangle", "")
	if len(res.Graph.Nodes) != 3 || len(res.Graph.Links) != 3 {
		t.Fatalf("expected triangle 3/3, got %d/%d", len(res.Graph.Nodes), len(res.Graph.Links))
	}

	if !strings.Contains(strings.Join(res.Notes, " "), "OSPF") {
		t.Errorf("expected OSPF note, got %v", res.Notes)
	}
}

func TestGenerateWithHosts(t *testing.T) {
	res, _ := Generate("2 srlinux routers each with a linux host", "")
	// 2 routers + 2 hosts = 4 nodes
	if len(res.Graph.Nodes) != 4 {
		t.Fatalf("expected 4 nodes, got %d", len(res.Graph.Nodes))
	}

	hosts := 0

	for _, n := range res.Graph.Nodes {
		if n.Kind == "linux" && strings.HasPrefix(n.Name, "host") {
			hosts++
		}
	}

	if hosts != 2 {
		t.Fatalf("expected 2 hosts, got %d", hosts)
	}
}

func TestGenerateUniqueInterfaces(t *testing.T) {
	res, _ := Generate("full mesh of 4 srlinux", "")

	// ensure no node reuses the same interface on two links
	seen := map[string]bool{}

	for _, l := range res.Graph.Links {
		a := l.Source + "/" + l.SourceEndpoint
		b := l.Target + "/" + l.TargetEndpoint

		if seen[a] {
			t.Errorf("duplicate interface %s", a)
		}

		if seen[b] {
			t.Errorf("duplicate interface %s", b)
		}

		seen[a] = true
		seen[b] = true
	}
}

func TestGenerateNameFromShape(t *testing.T) {
	res, _ := Generate("ring of 3 srlinux", "")
	if res.Graph.Name == "" {
		t.Fatal("expected a generated lab name")
	}
}
