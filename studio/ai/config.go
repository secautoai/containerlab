// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"fmt"
	"sort"
	"strings"

	"github.com/srl-labs/containerlab/studio/model"
)

// IfaceAddr is an interface with its assigned CIDR address.
type IfaceAddr struct {
	Iface string `json:"iface"`
	CIDR  string `json:"cidr"`
	Peer  string `json:"peer,omitempty"`
}

// NodeAddressing is the addressing plan for a single node.
type NodeAddressing struct {
	Node       string      `json:"node"`
	Loopback   string      `json:"loopback"`
	Interfaces []IfaceAddr `json:"interfaces"`
}

// AddressPlan is the addressing plan for a whole topology.
type AddressPlan struct {
	Protocol string           `json:"protocol"`
	Nodes    []NodeAddressing `json:"nodes"`
	Summary  string           `json:"summary"`
}

// AssignAddressing computes a deterministic IPv4 addressing plan for a topology:
// a /30 per point-to-point link (source=.1, target=.2) drawn from 10.<n>.<m>.0/30,
// and a loopback /32 per node from 10.255.0.0/24. It does not mutate the graph.
func AssignAddressing(g *model.Graph) *AddressPlan {
	plan := &AddressPlan{}

	// Stable node ordering for deterministic loopbacks.
	names := make([]string, 0, len(g.Nodes))
	for _, n := range g.Nodes {
		names = append(names, n.Name)
	}

	sort.Strings(names)

	byNode := map[string]*NodeAddressing{}
	for i, name := range names {
		na := &NodeAddressing{
			Node:     name,
			Loopback: fmt.Sprintf("10.255.0.%d/32", clampOctet(i+1)),
		}
		byNode[name] = na
	}

	// Assign a /30 per link.
	for i, l := range g.Links {
		if l == nil || l.Source == "" || l.Target == "" {
			continue
		}

		hostA, hostB := linkHosts(i)
		aIP := hostA + "/30"
		bIP := hostB + "/30"

		if na, ok := byNode[l.Source]; ok {
			na.Interfaces = append(na.Interfaces, IfaceAddr{
				Iface: l.SourceEndpoint, CIDR: aIP, Peer: l.Target,
			})
		}

		if na, ok := byNode[l.Target]; ok {
			na.Interfaces = append(na.Interfaces, IfaceAddr{
				Iface: l.TargetEndpoint, CIDR: bIP, Peer: l.Source,
			})
		}
	}

	for _, name := range names {
		plan.Nodes = append(plan.Nodes, *byNode[name])
	}

	links := len(g.Links)
	plan.Summary = fmt.Sprintf("assigned /30s to %d links and loopbacks to %d nodes",
		links, len(names))

	return plan
}

// linkHosts returns the two usable host addresses of the /30 for link index i,
// drawn sequentially from 10.0.0.0/8 (each /30 is 4 addresses apart):
// i=0 -> 10.0.0.1, 10.0.0.2 ; i=1 -> 10.0.0.5, 10.0.0.6 ; etc.
func linkHosts(i int) (a, b string) {
	base := uint32(10)<<24 + uint32(i)*4 //nolint:mnd // 10.0.0.0 + i*4

	return ipString(base + 1), ipString(base + 2)
}

// ipString formats a uint32 as a dotted-quad IPv4 address.
func ipString(v uint32) string {
	return fmt.Sprintf("%d.%d.%d.%d", (v>>24)&0xff, (v>>16)&0xff, (v>>8)&0xff, v&0xff)
}

func clampOctet(n int) int {
	if n < 1 {
		return 1
	}

	if n > 254 {
		return 254
	}

	return n
}

// AutoConfigure assigns addressing and writes per-node startup commands into the
// graph. Linux nodes receive `exec` commands to bring up interfaces and assign
// addresses (idiomatic containerlab, see lab-examples). When protocol is "ospf"
// or "bgp", FRR-style vtysh commands are appended (effective on FRR images).
//
// The graph is mutated in place. The returned plan describes the addressing.
func AutoConfigure(g *model.Graph, protocol string) (*AddressPlan, error) {
	if g == nil {
		return nil, fmt.Errorf("nil graph")
	}

	protocol = strings.ToLower(strings.TrimSpace(protocol))
	plan := AssignAddressing(g)
	plan.Protocol = protocol

	byNode := map[string]*NodeAddressing{}
	for i := range plan.Nodes {
		byNode[plan.Nodes[i].Node] = &plan.Nodes[i]
	}

	for _, n := range g.Nodes {
		na, ok := byNode[n.Name]
		if !ok {
			continue
		}

		// Only linux nodes get exec-based config; NOS kinds would need
		// startup-config files, which are out of scope for auto-config.
		if n.Kind != "linux" {
			continue
		}

		n.Exec = linuxExec(na, protocol)
	}

	switch protocol {
	case "ospf", "bgp":
		plan.Summary += fmt.Sprintf("; generated %s config for linux/FRR nodes", strings.ToUpper(protocol))
	}

	return plan, nil
}

// linuxExec builds the exec command list for a linux node given its addressing.
func linuxExec(na *NodeAddressing, protocol string) []string {
	var cmds []string

	// loopback
	if na.Loopback != "" {
		cmds = append(cmds, fmt.Sprintf("ip addr add %s dev lo", na.Loopback))
	}

	for _, ifa := range na.Interfaces {
		if ifa.Iface == "" {
			continue
		}

		cmds = append(cmds,
			fmt.Sprintf("ip link set %s up", ifa.Iface),
			fmt.Sprintf("ip addr add %s dev %s", ifa.CIDR, ifa.Iface),
		)
	}

	switch protocol {
	case "ospf":
		cmds = append(cmds, frrOSPF(na)...)
	case "bgp":
		cmds = append(cmds, frrBGP(na)...)
	}

	return cmds
}

// frrOSPF returns vtysh commands enabling OSPF on all of the node's networks.
func frrOSPF(na *NodeAddressing) []string {
	lo := strings.TrimSuffix(na.Loopback, "/32")

	cmds := []string{
		"vtysh -c 'conf t' -c 'router ospf' -c 'ospf router-id " + lo + "'",
	}

	// advertise loopback + each p2p network into area 0
	nets := append([]string{na.Loopback}, ifaceCIDRs(na)...)
	for _, net := range nets {
		cmds = append(cmds,
			fmt.Sprintf("vtysh -c 'conf t' -c 'router ospf' -c 'network %s area 0'", net))
	}

	return cmds
}

// frrBGP returns vtysh commands establishing a simple eBGP-less iBGP scaffold
// (router-id + networks). Neighbor setup is left to the user/AI follow-up.
func frrBGP(na *NodeAddressing) []string {
	lo := strings.TrimSuffix(na.Loopback, "/32")

	cmds := []string{
		fmt.Sprintf("vtysh -c 'conf t' -c 'router bgp 65000' -c 'bgp router-id %s'", lo),
	}

	for _, net := range append([]string{na.Loopback}, ifaceCIDRs(na)...) {
		cmds = append(cmds,
			fmt.Sprintf("vtysh -c 'conf t' -c 'router bgp 65000' -c 'address-family ipv4 unicast' -c 'network %s'", net))
	}

	return cmds
}

func ifaceCIDRs(na *NodeAddressing) []string {
	out := make([]string, 0, len(na.Interfaces))
	for _, ifa := range na.Interfaces {
		out = append(out, ifa.CIDR)
	}

	return out
}
