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
