// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

// clabFile mirrors the subset of the containerlab *.clab.yml schema that
// ClabStudio reads and writes. It is intentionally a focused schema: it covers
// the fields the canvas edits while remaining compatible with real topologies.
type clabFile struct {
	Name     string       `yaml:"name"`
	Mgmt     *clabMgmt    `yaml:"mgmt,omitempty"`
	Topology clabTopology `yaml:"topology"`
}

type clabMgmt struct {
	Network    string `yaml:"network,omitempty"`
	IPv4Subnet string `yaml:"ipv4-subnet,omitempty"`
	IPv6Subnet string `yaml:"ipv6-subnet,omitempty"`
}

type clabTopology struct {
	Defaults *clabNode            `yaml:"defaults,omitempty"`
	Kinds    map[string]*clabNode `yaml:"kinds,omitempty"`
	Groups   map[string]*clabNode `yaml:"groups,omitempty"`
	Nodes    map[string]*clabNode `yaml:"nodes,omitempty"`
	Links    []*clabLink          `yaml:"links,omitempty"`
}

type clabNode struct {
	Kind          string            `yaml:"kind,omitempty"`
	Image         string            `yaml:"image,omitempty"`
	Type          string            `yaml:"type,omitempty"`
	Group         string            `yaml:"group,omitempty"`
	StartupConfig string            `yaml:"startup-config,omitempty"`
	MgmtIPv4      string            `yaml:"mgmt-ipv4,omitempty"`
	MgmtIPv6      string            `yaml:"mgmt-ipv6,omitempty"`
	Binds         []string          `yaml:"binds,omitempty"`
	Exec          []string          `yaml:"exec,omitempty"`
	Env           map[string]string `yaml:"env,omitempty"`
	Labels        map[string]string `yaml:"labels,omitempty"`
}

type clabLink struct {
	Type      string   `yaml:"type,omitempty"`
	Endpoints []string `yaml:"endpoints,omitempty"`
	MTU       int      `yaml:"mtu,omitempty"`
}

// ClabYAMLToGraph parses a containerlab topology YAML document into the UI graph
// model. Node kind/image/type are resolved through the group -> kind -> defaults
// inheritance chain so the UI always shows effective values.
func ClabYAMLToGraph(data []byte) (*Graph, error) {
	var f clabFile
	if err := yaml.Unmarshal(data, &f); err != nil {
		return nil, fmt.Errorf("failed to parse topology YAML: %w", err)
	}

	g := &Graph{
		Name:  f.Name,
		Nodes: []*Node{},
		Links: []*Link{},
	}

	if f.Mgmt != nil {
		g.Mgmt = &MgmtNet{
			Network:    f.Mgmt.Network,
			IPv4Subnet: f.Mgmt.IPv4Subnet,
			IPv6Subnet: f.Mgmt.IPv6Subnet,
		}
	}

	// stable ordering of node names for deterministic output
	names := make([]string, 0, len(f.Topology.Nodes))
	for name := range f.Topology.Nodes {
		names = append(names, name)
	}

	sort.Strings(names)

	for _, name := range names {
		raw := f.Topology.Nodes[name]
		if raw == nil {
			raw = &clabNode{}
		}

		eff := resolveNode(raw, f.Topology)

		n := &Node{
			Name:          name,
			Kind:          eff.Kind,
			Image:         eff.Image,
			Type:          eff.Type,
			Group:         raw.Group,
			MgmtIPv4:      raw.MgmtIPv4,
			MgmtIPv6:      raw.MgmtIPv6,
			StartupConfig: raw.StartupConfig,
			Binds:         raw.Binds,
			Exec:          raw.Exec,
			Env:           raw.Env,
			Labels:        raw.Labels,
		}

		applyPositionFromLabels(n)

		g.Nodes = append(g.Nodes, n)
	}

	for _, l := range f.Topology.Links {
		link := briefToLink(l)
		if link != nil {
			g.Links = append(g.Links, link)
		}
	}

	return g, nil
}

// resolveNode merges a node definition with its group, kind and defaults so the
// returned node has effective kind/image/type values.
func resolveNode(n *clabNode, topo clabTopology) clabNode {
	eff := *n

	// group inheritance
	if n.Group != "" && topo.Groups != nil {
		if grp, ok := topo.Groups[n.Group]; ok && grp != nil {
			mergeNode(&eff, grp)
		}
	}

	// kind inheritance keyed by the (already-resolved) kind name
	if eff.Kind != "" && topo.Kinds != nil {
		if knd, ok := topo.Kinds[eff.Kind]; ok && knd != nil {
			mergeNode(&eff, knd)
		}
	}

	if topo.Defaults != nil {
		mergeNode(&eff, topo.Defaults)
	}

	return eff
}

// mergeNode fills empty fields of dst from src (dst takes precedence).
func mergeNode(dst, src *clabNode) {
	if dst.Kind == "" {
		dst.Kind = src.Kind
	}

	if dst.Image == "" {
		dst.Image = src.Image
	}

	if dst.Type == "" {
		dst.Type = src.Type
	}
}

func applyPositionFromLabels(n *Node) {
	if n.Labels == nil {
		return
	}

	if v, ok := n.Labels[LabelPosX]; ok {
		if f, err := strconv.ParseFloat(v, 64); err == nil {
			n.Position.X = f
		}
	}

	if v, ok := n.Labels[LabelPosY]; ok {
		if f, err := strconv.ParseFloat(v, 64); err == nil {
			n.Position.Y = f
		}
	}

	if v, ok := n.Labels[LabelIcon]; ok {
		n.Icon = v
	}
}

// briefToLink converts a containerlab (brief) link definition into a UI link.
// Only point-to-point links with two endpoints are represented on the canvas.
func briefToLink(l *clabLink) *Link {
	if l == nil || len(l.Endpoints) != 2 {
		return nil
	}

	src, srcEp := splitEndpoint(l.Endpoints[0])
	dst, dstEp := splitEndpoint(l.Endpoints[1])

	if src == "" || dst == "" {
		return nil
	}

	return &Link{
		Source:         src,
		SourceEndpoint: srcEp,
		Target:         dst,
		TargetEndpoint: dstEp,
		MTU:            l.MTU,
	}
}

func splitEndpoint(ep string) (node, iface string) {
	ep = strings.TrimSpace(ep)

	parts := strings.SplitN(ep, ":", 2)
	if len(parts) == 2 {
		return strings.TrimSpace(parts[0]), strings.TrimSpace(parts[1])
	}

	return strings.TrimSpace(parts[0]), ""
}

// GraphToClabYAML serializes the UI graph model into a containerlab topology
// YAML document. UI positions/icons are persisted into node labels so the layout
// survives a save/load round-trip.
func GraphToClabYAML(g *Graph) ([]byte, error) {
	if g == nil {
		return nil, fmt.Errorf("nil graph")
	}

	if strings.TrimSpace(g.Name) == "" {
		return nil, fmt.Errorf("lab name is required")
	}

	f := clabFile{
		Name: g.Name,
		Topology: clabTopology{
			Nodes: map[string]*clabNode{},
			Links: []*clabLink{},
		},
	}

	if g.Mgmt != nil && (g.Mgmt.Network != "" || g.Mgmt.IPv4Subnet != "" || g.Mgmt.IPv6Subnet != "") {
		f.Mgmt = &clabMgmt{
			Network:    g.Mgmt.Network,
			IPv4Subnet: g.Mgmt.IPv4Subnet,
			IPv6Subnet: g.Mgmt.IPv6Subnet,
		}
	}

	for _, n := range g.Nodes {
		if n == nil || n.Name == "" {
			continue
		}

		labels := persistPositionToLabels(n)

		f.Topology.Nodes[n.Name] = &clabNode{
			Kind:          n.Kind,
			Image:         n.Image,
			Type:          n.Type,
			Group:         n.Group,
			StartupConfig: n.StartupConfig,
			MgmtIPv4:      n.MgmtIPv4,
			MgmtIPv6:      n.MgmtIPv6,
			Binds:         n.Binds,
			Exec:          n.Exec,
			Env:           n.Env,
			Labels:        labels,
		}
	}

	for _, l := range g.Links {
		if l == nil || l.Source == "" || l.Target == "" {
			continue
		}

		f.Topology.Links = append(f.Topology.Links, &clabLink{
			Endpoints: []string{
				joinEndpoint(l.Source, l.SourceEndpoint),
				joinEndpoint(l.Target, l.TargetEndpoint),
			},
			MTU: l.MTU,
		})
	}

	out, err := yaml.Marshal(&f)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal topology YAML: %w", err)
	}

	return out, nil
}

// persistPositionToLabels returns a copy of the node's labels enriched with the
// UI position/icon so they survive serialization.
func persistPositionToLabels(n *Node) map[string]string {
	labels := map[string]string{}
	for k, v := range n.Labels {
		labels[k] = v
	}

	if n.Position.X != 0 || n.Position.Y != 0 {
		labels[LabelPosX] = strconv.FormatFloat(n.Position.X, 'f', -1, 64)
		labels[LabelPosY] = strconv.FormatFloat(n.Position.Y, 'f', -1, 64)
	}

	if n.Icon != "" {
		labels[LabelIcon] = n.Icon
	}

	if len(labels) == 0 {
		return nil
	}

	return labels
}

func joinEndpoint(node, iface string) string {
	if iface == "" {
		return node
	}

	return node + ":" + iface
}
