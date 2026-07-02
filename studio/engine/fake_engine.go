// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"sync"

	"github.com/srl-labs/containerlab/studio/model"
)

// FakeEngine is an in-memory Engine implementation used for tests and for
// running ClabStudio without a container runtime (design-only mode). It never
// touches Docker/containerlab.
type FakeEngine struct {
	mu       sync.RWMutex
	labs     map[string]*model.Graph
	deployed map[string]bool
	// RuntimeUp toggles whether lifecycle operations are permitted.
	RuntimeUp bool
}

// NewFakeEngine returns an empty in-memory engine. When runtimeUp is true,
// deploy/destroy/exec succeed against the in-memory state.
func NewFakeEngine(runtimeUp bool) *FakeEngine {
	return &FakeEngine{
		labs:      map[string]*model.Graph{},
		deployed:  map[string]bool{},
		RuntimeUp: runtimeUp,
	}
}

var _ Engine = (*FakeEngine)(nil)

// Capabilities implements Engine.
func (e *FakeEngine) Capabilities(_ context.Context) Capabilities {
	c := Capabilities{RuntimeAvailable: e.RuntimeUp, Runtime: "fake"}
	if !e.RuntimeUp {
		c.Reason = "fake engine runtime is disabled"
	}

	return c
}

// ListLabs implements Engine.
func (e *FakeEngine) ListLabs(_ context.Context) ([]LabSummary, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	out := make([]LabSummary, 0, len(e.labs))

	for name, g := range e.labs {
		out = append(out, LabSummary{
			Name:      name,
			NodeCount: len(g.Nodes),
			Deployed:  e.deployed[name],
			State:     stateFor(e.deployed[name]),
		})
	}

	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })

	return out, nil
}

func stateFor(deployed bool) string {
	if deployed {
		return "running"
	}

	return "defined"
}

// GetLab implements Engine.
func (e *FakeEngine) GetLab(_ context.Context, name string) (*model.Graph, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	g, ok := e.labs[name]
	if !ok {
		return nil, fmt.Errorf("lab %q not found", name)
	}

	return g, nil
}

// SaveLab implements Engine.
func (e *FakeEngine) SaveLab(_ context.Context, g *model.Graph) error {
	if g == nil || strings.TrimSpace(g.Name) == "" {
		return fmt.Errorf("lab name is required")
	}

	e.mu.Lock()
	defer e.mu.Unlock()

	e.labs[g.Name] = g

	return nil
}

// DeleteLab implements Engine.
func (e *FakeEngine) DeleteLab(_ context.Context, name string) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	if e.deployed[name] {
		return fmt.Errorf("lab %q is deployed; destroy it first", name)
	}

	if _, ok := e.labs[name]; !ok {
		return fmt.Errorf("lab %q not found", name)
	}

	delete(e.labs, name)

	return nil
}

// RenderYAML implements Engine.
func (e *FakeEngine) RenderYAML(_ context.Context, name string) ([]byte, error) {
	e.mu.RLock()
	g, ok := e.labs[name]
	e.mu.RUnlock()

	if !ok {
		return nil, fmt.Errorf("lab %q not found", name)
	}

	return model.GraphToClabYAML(g)
}

// Deploy implements Engine.
func (e *FakeEngine) Deploy(ctx context.Context, name string) (*LabStatus, error) {
	if !e.RuntimeUp {
		return nil, ErrRuntimeUnavailable
	}

	e.mu.Lock()
	if _, ok := e.labs[name]; !ok {
		e.mu.Unlock()
		return nil, fmt.Errorf("lab %q not found", name)
	}

	e.deployed[name] = true
	e.mu.Unlock()

	return e.Status(ctx, name)
}

// Destroy implements Engine.
func (e *FakeEngine) Destroy(_ context.Context, name string, _ bool) error {
	if !e.RuntimeUp {
		return ErrRuntimeUnavailable
	}

	e.mu.Lock()
	defer e.mu.Unlock()

	e.deployed[name] = false

	return nil
}

// Status implements Engine.
func (e *FakeEngine) Status(_ context.Context, name string) (*LabStatus, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	g, ok := e.labs[name]
	if !ok {
		return nil, fmt.Errorf("lab %q not found", name)
	}

	deployed := e.deployed[name]
	st := &LabStatus{Name: name, Deployed: deployed}

	for _, n := range g.Nodes {
		state := "stopped"
		if deployed {
			state = "running"
		}

		st.Nodes = append(st.Nodes, NodeStatus{
			Name:  n.Name,
			Kind:  n.Kind,
			Image: n.Image,
			State: state,
		})
	}

	return st, nil
}

// Exec implements Engine.
func (e *FakeEngine) Exec(_ context.Context, lab, node, cmd string) (*ExecResult, error) {
	if !e.RuntimeUp {
		return nil, ErrRuntimeUnavailable
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return nil, fmt.Errorf("lab %q is not deployed", lab)
	}

	return &ExecResult{
		Node:       node,
		Cmd:        cmd,
		ReturnCode: 0,
		Stdout:     fmt.Sprintf("fake output for %q on %s/%s", cmd, lab, node),
	}, nil
}

// ConsoleTarget implements Engine.
func (e *FakeEngine) ConsoleTarget(_ context.Context, lab, node string) (*ConsoleTarget, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	g, ok := e.labs[lab]
	if !ok {
		return nil, fmt.Errorf("%w: lab %q", ErrNotFound, lab)
	}

	var kind string

	for _, n := range g.Nodes {
		if n.Name == node {
			kind = n.Kind
			break
		}
	}

	return &ConsoleTarget{
		Container: fmt.Sprintf("clab-%s-%s", lab, node),
		Cmd:       ConsoleCommandForKind(kind),
	}, nil
}
