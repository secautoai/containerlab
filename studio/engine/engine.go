// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package engine abstracts the lab orchestration backend used by ClabStudio.
//
// The Engine interface decouples the HTTP/WebSocket layer from containerlab's
// core so that the API can be unit-tested with an in-memory fake (no Docker
// runtime required), while the production implementation drives real
// containerlab labs via the core.CLab APIs.
package engine

import (
	"context"
	"fmt"

	"github.com/srl-labs/containerlab/studio/model"
)

// LabSummary is a lightweight description of a lab used in list views.
type LabSummary struct {
	Name string `json:"name"`
	// Path is the on-disk topology file path (may be empty for unsaved labs).
	Path string `json:"path,omitempty"`
	// NodeCount is the number of nodes defined in the topology.
	NodeCount int `json:"nodeCount"`
	// Deployed indicates whether any containers for the lab are running.
	Deployed bool `json:"deployed"`
	// Owner is the lab owner label, when known.
	Owner string `json:"owner,omitempty"`
	// State is a coarse lifecycle state: "defined", "running", "partial".
	State string `json:"state"`
}

// NodeStatus captures runtime information about a single node.
type NodeStatus struct {
	Name        string `json:"name"`
	Kind        string `json:"kind"`
	Image       string `json:"image,omitempty"`
	State       string `json:"state"`
	IPv4Address string `json:"ipv4Address,omitempty"`
	IPv6Address string `json:"ipv6Address,omitempty"`
}

// LabStatus is the runtime status of a whole lab.
type LabStatus struct {
	Name     string       `json:"name"`
	Deployed bool         `json:"deployed"`
	Nodes    []NodeStatus `json:"nodes"`
}

// ExecResult is the result of running a command on a node.
type ExecResult struct {
	Node       string `json:"node"`
	Cmd        string `json:"cmd"`
	ReturnCode int    `json:"returnCode"`
	Stdout     string `json:"stdout"`
	Stderr     string `json:"stderr"`
}

// RuntimeAvailable reports whether a container runtime is usable. When false,
// design/AI/YAML operations still work but lifecycle actions will error.
type Capabilities struct {
	RuntimeAvailable bool   `json:"runtimeAvailable"`
	Runtime          string `json:"runtime"`
	Reason           string `json:"reason,omitempty"`
}

// Engine is the orchestration backend contract used by the API layer.
type Engine interface {
	// Capabilities reports runtime availability and other backend features.
	Capabilities(ctx context.Context) Capabilities

	// ListLabs returns all known labs (saved on disk and/or deployed).
	ListLabs(ctx context.Context) ([]LabSummary, error)

	// GetLab returns the topology graph for a lab by name.
	GetLab(ctx context.Context, name string) (*model.Graph, error)

	// SaveLab persists a lab's topology graph to disk, creating it if needed.
	SaveLab(ctx context.Context, g *model.Graph) error

	// DeleteLab removes a lab definition from disk (must not be deployed).
	DeleteLab(ctx context.Context, name string) error

	// RenderYAML returns the containerlab YAML for a lab graph.
	RenderYAML(ctx context.Context, name string) ([]byte, error)

	// Deploy deploys a lab. Requires an available runtime.
	Deploy(ctx context.Context, name string) (*LabStatus, error)

	// Destroy tears down a deployed lab.
	Destroy(ctx context.Context, name string, cleanup bool) error

	// Status returns runtime status of a lab.
	Status(ctx context.Context, name string) (*LabStatus, error)

	// Exec runs a command on a node and returns the result.
	Exec(ctx context.Context, lab, node, cmd string) (*ExecResult, error)

	// ConsoleTarget resolves the container name and default interactive command
	// for a node so the server can attach a browser console to it.
	ConsoleTarget(ctx context.Context, lab, node string) (*ConsoleTarget, error)

	// Validate runs an end-to-end reachability check across a deployed lab.
	Validate(ctx context.Context, lab string) (*ValidationReport, error)

	// NodeLifecycle performs a per-node action: "start", "stop" or "restart".
	NodeLifecycle(ctx context.Context, lab, node, action string) error

	// SetImpairment applies (or clears) netem link impairments on a node's
	// interface. Passing an all-zero params clears the impairments.
	SetImpairment(ctx context.Context, lab, node, iface string, params ImpairmentParams) error

	// SaveConfigs persists the running configuration of a deployed lab's nodes
	// (equivalent to `containerlab save`).
	SaveConfigs(ctx context.Context, lab string) error

	// CloneLab copies a lab's topology to a new lab name (dst must not exist).
	CloneLab(ctx context.Context, src, dst string) error

	// RenameLab renames a lab (new must not exist; lab must not be deployed).
	RenameLab(ctx context.Context, oldName, newName string) error

	// Throughput runs an iperf3 test from one node to another in a deployed lab.
	Throughput(ctx context.Context, lab, from, to string) (*ThroughputResult, error)

	// Capture captures `count` packets on a node interface and returns the raw
	// pcap bytes (requires tcpdump in the node image and a running lab).
	Capture(ctx context.Context, lab, node, iface string, count int) ([]byte, error)
}

// ImpairmentParams describes netem link impairments for an interface.
type ImpairmentParams struct {
	// DelayMs is the added one-way latency in milliseconds.
	DelayMs uint `json:"delayMs"`
	// JitterMs is the latency variation in milliseconds (requires DelayMs > 0).
	JitterMs uint `json:"jitterMs"`
	// LossPct is the packet loss percentage (0-100).
	LossPct float64 `json:"lossPct"`
	// RateKbit rate-limits the interface in kbit/s (0 = unlimited).
	RateKbit uint64 `json:"rateKbit"`
	// CorruptionPct is the packet corruption percentage (0-100).
	CorruptionPct float64 `json:"corruptionPct"`
}

// Validate checks that the impairment parameters are within valid ranges.
func (p ImpairmentParams) Validate() error {
	if p.LossPct < 0 || p.LossPct > 100 {
		return fmt.Errorf("packet loss must be between 0 and 100")
	}

	if p.CorruptionPct < 0 || p.CorruptionPct > 100 {
		return fmt.Errorf("corruption must be between 0 and 100")
	}

	if p.JitterMs != 0 && p.DelayMs == 0 {
		return fmt.Errorf("jitter cannot be set without a delay")
	}

	return nil
}

// ValidActions are the accepted NodeLifecycle actions.
func ValidActions() []string { return []string{"start", "stop", "restart"} }

// ConsoleTarget describes how to open an interactive console into a node.
type ConsoleTarget struct {
	// Container is the runtime container name (e.g. clab-<lab>-<node>).
	Container string `json:"container"`
	// Cmd is the default interactive command (vendor CLI or shell).
	Cmd []string `json:"cmd"`
}

// ConsoleCommandForKind returns a sensible default interactive command for a
// containerlab kind. Network OSes get their native CLI; everything else falls
// back to a POSIX shell (with bash preferred when present, handled at attach).
func ConsoleCommandForKind(kind string) []string {
	switch kind {
	case "nokia_srlinux":
		return []string{"sr_cli"}
	case "arista_ceos":
		return []string{"Cli"}
	case "juniper_crpd":
		return []string{"cli"}
	case "cvx":
		return []string{"bash"}
	default:
		return []string{"/bin/sh"}
	}
}
