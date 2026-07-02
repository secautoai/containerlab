// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"sync"

	"github.com/srl-labs/containerlab/studio/model"
)

// cloneGraph returns a deep copy of a graph via a JSON round-trip.
func cloneGraph(g *model.Graph) *model.Graph {
	b, _ := json.Marshal(g)

	var c model.Graph

	_ = json.Unmarshal(b, &c)

	return &c
}

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

// Validate implements Engine (in-memory: all pairs reachable when deployed).
func (e *FakeEngine) Validate(_ context.Context, lab string) (*ValidationReport, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	g, ok := e.labs[lab]
	if !ok {
		return nil, fmt.Errorf("%w: lab %q", ErrNotFound, lab)
	}

	report := &ValidationReport{Lab: lab, Deployed: e.deployed[lab]}
	if !report.Deployed {
		report.summarize()
		return report, nil
	}

	for _, a := range g.Nodes {
		for _, b := range g.Nodes {
			if a.Name == b.Name {
				continue
			}

			report.Checks = append(report.Checks, ReachabilityCheck{
				From: a.Name, To: b.Name, Target: "fake", OK: true,
			})
			report.Passed++
		}
	}

	report.summarize()

	return report, nil
}

// NodeLifecycle implements Engine (in-memory no-op when deployed).
func (e *FakeEngine) NodeLifecycle(_ context.Context, lab, _, action string) error {
	if !e.RuntimeUp {
		return ErrRuntimeUnavailable
	}

	if !isValidAction(action) {
		return fmt.Errorf("invalid action %q", action)
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return fmt.Errorf("lab %q is not deployed", lab)
	}

	return nil
}

// SetImpairment implements Engine (in-memory: validate + require deployed).
func (e *FakeEngine) SetImpairment(_ context.Context, lab, _, iface string, params ImpairmentParams) error {
	if err := params.Validate(); err != nil {
		return err
	}

	if iface == "" {
		return fmt.Errorf("interface is required")
	}

	if !e.RuntimeUp {
		return ErrRuntimeUnavailable
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return fmt.Errorf("lab %q is not deployed", lab)
	}

	return nil
}

// CloneLab implements Engine.
func (e *FakeEngine) CloneLab(_ context.Context, src, dst string) error {
	if err := validateLabName(dst); err != nil {
		return err
	}

	e.mu.Lock()
	defer e.mu.Unlock()

	g, ok := e.labs[src]
	if !ok {
		return fmt.Errorf("%w: lab %q", ErrNotFound, src)
	}

	if _, exists := e.labs[dst]; exists {
		return fmt.Errorf("lab %q already exists", dst)
	}

	clone := cloneGraph(g)
	clone.Name = dst
	e.labs[dst] = clone

	return nil
}

// RenameLab implements Engine.
func (e *FakeEngine) RenameLab(_ context.Context, oldName, newName string) error {
	if err := validateLabName(newName); err != nil {
		return err
	}

	e.mu.Lock()
	defer e.mu.Unlock()

	g, ok := e.labs[oldName]
	if !ok {
		return fmt.Errorf("%w: lab %q", ErrNotFound, oldName)
	}

	if _, exists := e.labs[newName]; exists {
		return fmt.Errorf("lab %q already exists", newName)
	}

	if e.deployed[oldName] {
		return fmt.Errorf("lab %q is deployed; destroy it before renaming", oldName)
	}

	g.Name = newName
	e.labs[newName] = g
	delete(e.labs, oldName)

	return nil
}

// Throughput implements Engine (in-memory: returns a stub result when deployed).
func (e *FakeEngine) Throughput(_ context.Context, lab, from, to string) (*ThroughputResult, error) {
	if !e.RuntimeUp {
		return nil, ErrRuntimeUnavailable
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return nil, fmt.Errorf("lab %q is not deployed", lab)
	}

	res := &ThroughputResult{
		From: from, To: to, Target: "fake",
		SentBitsPerSec: 1e9, RecvBitsPerSec: 1e9,
		SentMbitsPerSec: 1000, RecvMbitsPerSec: 1000,
	}
	res.Summary = "1000.0 Mbit/s sent, 1000.0 Mbit/s received, 0 retransmits"

	return res, nil
}

// Capture implements Engine (in-memory: returns a valid minimal pcap when
// deployed and inputs are valid).
func (e *FakeEngine) Capture(_ context.Context, lab, _, iface string, count int) ([]byte, error) {
	if _, err := buildCaptureCmd(iface, count); err != nil {
		return nil, err
	}

	if !e.RuntimeUp {
		return nil, ErrRuntimeUnavailable
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return nil, fmt.Errorf("lab %q is not deployed", lab)
	}

	return minimalPcap(), nil
}

// SaveConfigs implements Engine (in-memory: require deployed).
func (e *FakeEngine) SaveConfigs(_ context.Context, lab string) error {
	if !e.RuntimeUp {
		return ErrRuntimeUnavailable
	}

	e.mu.RLock()
	defer e.mu.RUnlock()

	if !e.deployed[lab] {
		return fmt.Errorf("lab %q is not deployed", lab)
	}

	return nil
}

func isValidAction(a string) bool {
	for _, v := range ValidActions() {
		if v == a {
			return true
		}
	}

	return false
}
