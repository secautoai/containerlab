// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package model defines the UI-friendly topology graph used by ClabStudio and
// lossless-enough converters to and from the containerlab *.clab.yml schema.
//
// The graph model is intentionally decoupled from containerlab's internal
// `types.Topology` so that the web UI has a stable, JSON-first contract while
// the converters keep compatibility with real containerlab topology files.
package model

// Graph is the UI-facing representation of a lab topology.
type Graph struct {
	// Name is the lab/topology name.
	Name string `json:"name"`
	// Mgmt holds optional management network settings.
	Mgmt *MgmtNet `json:"mgmt,omitempty"`
	// Nodes is the list of nodes in the topology.
	Nodes []*Node `json:"nodes"`
	// Links is the list of point-to-point links between nodes.
	Links []*Link `json:"links"`
}

// MgmtNet captures the management network configuration exposed to the UI.
type MgmtNet struct {
	Network    string `json:"network,omitempty"`
	IPv4Subnet string `json:"ipv4Subnet,omitempty"`
	IPv6Subnet string `json:"ipv6Subnet,omitempty"`
}

// Node is a single node on the canvas.
type Node struct {
	// Name is the unique node name within the topology.
	Name string `json:"name"`
	// Kind is the containerlab kind (e.g. linux, nokia_srlinux, arista_ceos).
	Kind string `json:"kind"`
	// Image is the container image reference. May be empty to use kind defaults.
	Image string `json:"image,omitempty"`
	// Type is a kind-specific type/variant (e.g. ixr-d3l for SR Linux).
	Type string `json:"type,omitempty"`
	// Group is an optional grouping used by containerlab groups/kinds.
	Group string `json:"group,omitempty"`
	// MgmtIPv4/MgmtIPv6 are optional static management addresses.
	MgmtIPv4 string `json:"mgmtIpv4,omitempty"`
	MgmtIPv6 string `json:"mgmtIpv6,omitempty"`
	// StartupConfig is an optional path to a startup config file.
	StartupConfig string `json:"startupConfig,omitempty"`
	// Binds are docker-style bind mounts.
	Binds []string `json:"binds,omitempty"`
	// Env are environment variables passed to the node.
	Env map[string]string `json:"env,omitempty"`
	// Exec are commands run after node deployment.
	Exec []string `json:"exec,omitempty"`
	// Labels are container labels; also used to persist UI position + icon.
	Labels map[string]string `json:"labels,omitempty"`
	// Position is the canvas position of the node (persisted via labels).
	Position Position `json:"position"`
	// Icon is an optional icon identifier for the UI.
	Icon string `json:"icon,omitempty"`
	// State is the runtime state (populated on inspect, not persisted to YAML).
	State string `json:"state,omitempty"`
	// IPv4Address/IPv6Address are runtime mgmt addresses (inspect only).
	IPv4Address string `json:"ipv4Address,omitempty"`
	IPv6Address string `json:"ipv6Address,omitempty"`
}

// Position is a 2D canvas coordinate.
type Position struct {
	X float64 `json:"x"`
	Y float64 `json:"y"`
}

// Link is a point-to-point connection between two node endpoints.
type Link struct {
	Source         string `json:"source"`
	SourceEndpoint string `json:"sourceEndpoint,omitempty"`
	Target         string `json:"target"`
	TargetEndpoint string `json:"targetEndpoint,omitempty"`
	MTU            int    `json:"mtu,omitempty"`
}

// Label keys used to persist UI-only metadata inside containerlab node labels so
// that a saved *.clab.yml round-trips through the canvas without losing layout.
const (
	LabelPosX = "graph-posX"
	LabelPosY = "graph-posY"
	LabelIcon = "graph-icon"
)
