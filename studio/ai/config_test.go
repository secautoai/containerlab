// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"fmt"
	"net"
	"strings"
	"testing"

	"github.com/srl-labs/containerlab/studio/model"
)

func twoLinuxGraph() *model.Graph {
	return &model.Graph{
		Name: "t",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "linux", Image: "frr"},
			{Name: "r2", Kind: "linux", Image: "frr"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "eth1", Target: "r2", TargetEndpoint: "eth1"},
		},
	}
}

func TestAssignAddressingDeterministic(t *testing.T) {
	g := twoLinuxGraph()

	p1 := AssignAddressing(g)
	p2 := AssignAddressing(g)

	if len(p1.Nodes) != 2 {
		t.Fatalf("expected 2 nodes, got %d", len(p1.Nodes))
	}

	// deterministic loopbacks
	if p1.Nodes[0].Loopback != p2.Nodes[0].Loopback {
		t.Errorf("addressing not deterministic")
	}

	if p1.Nodes[0].Loopback != "10.255.0.1/32" || p1.Nodes[1].Loopback != "10.255.0.2/32" {
		t.Errorf("unexpected loopbacks: %q %q", p1.Nodes[0].Loopback, p1.Nodes[1].Loopback)
	}

	// each node has one interface address in the same /30
	if len(p1.Nodes[0].Interfaces) != 1 || len(p1.Nodes[1].Interfaces) != 1 {
		t.Fatalf("expected 1 interface per node")
	}

	a := p1.Nodes[0].Interfaces[0].CIDR
	b := p1.Nodes[1].Interfaces[0].CIDR

	if !strings.HasSuffix(a, ".1/30") || !strings.HasSuffix(b, ".2/30") {
		t.Errorf("expected .1/.2 addressing, got %q %q", a, b)
	}

	// addresses must be valid dotted-quad CIDRs (regression: 5-octet bug)
	for _, cidr := range []string{a, b, p1.Nodes[0].Loopback} {
		if _, _, err := net.ParseCIDR(cidr); err != nil {
			t.Errorf("invalid CIDR %q: %v", cidr, err)
		}
	}
}

func TestAddressingManyLinksValid(t *testing.T) {
	// build a linear chain of linux nodes with many links and ensure every
	// generated address parses cleanly.
	g := &model.Graph{Name: "chain"}
	for i := 0; i < 70; i++ {
		g.Nodes = append(g.Nodes, &model.Node{Name: fmt.Sprintf("n%d", i), Kind: "linux"})
	}
	for i := 0; i < 69; i++ {
		g.Links = append(g.Links, &model.Link{
			Source: fmt.Sprintf("n%d", i), SourceEndpoint: "eth1",
			Target: fmt.Sprintf("n%d", i+1), TargetEndpoint: "eth2",
		})
	}

	plan := AssignAddressing(g)
	for _, n := range plan.Nodes {
		for _, ifa := range n.Interfaces {
			if _, _, err := net.ParseCIDR(ifa.CIDR); err != nil {
				t.Fatalf("invalid CIDR %q: %v", ifa.CIDR, err)
			}
		}
	}
}

func TestAutoConfigureLinuxExec(t *testing.T) {
	g := twoLinuxGraph()

	plan, err := AutoConfigure(g, "none")
	if err != nil {
		t.Fatalf("auto configure: %v", err)
	}

	if plan.Protocol != "none" {
		t.Errorf("unexpected protocol %q", plan.Protocol)
	}

	for _, n := range g.Nodes {
		if len(n.Exec) == 0 {
			t.Fatalf("node %s got no exec commands", n.Name)
		}

		joined := strings.Join(n.Exec, "\n")
		if !strings.Contains(joined, "ip addr add") || !strings.Contains(joined, "dev eth1") {
			t.Errorf("node %s missing interface config: %v", n.Name, n.Exec)
		}

		if !strings.Contains(joined, "dev lo") {
			t.Errorf("node %s missing loopback config", n.Name)
		}
	}
}

func TestAutoConfigureOSPF(t *testing.T) {
	g := twoLinuxGraph()

	_, err := AutoConfigure(g, "ospf")
	if err != nil {
		t.Fatalf("auto configure: %v", err)
	}

	joined := strings.Join(g.Nodes[0].Exec, "\n")
	if !strings.Contains(joined, "router ospf") || !strings.Contains(joined, "area 0") {
		t.Errorf("expected OSPF config, got: %v", g.Nodes[0].Exec)
	}
}

func TestAutoConfigureSkipsNonLinux(t *testing.T) {
	g := &model.Graph{
		Name: "t",
		Nodes: []*model.Node{
			{Name: "srl1", Kind: "nokia_srlinux"},
			{Name: "srl2", Kind: "nokia_srlinux"},
		},
		Links: []*model.Link{
			{Source: "srl1", SourceEndpoint: "e1-1", Target: "srl2", TargetEndpoint: "e1-1"},
		},
	}

	_, err := AutoConfigure(g, "ospf")
	if err != nil {
		t.Fatalf("auto configure: %v", err)
	}

	// NOS nodes should not receive exec-based config
	for _, n := range g.Nodes {
		if len(n.Exec) != 0 {
			t.Errorf("non-linux node %s should not get exec config, got %v", n.Name, n.Exec)
		}
	}
}
