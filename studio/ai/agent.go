// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package ai implements the ClabStudio Copilot: an agent that turns
// plain-English requests into topologies (and, in later milestones, configs and
// validations). It works fully offline via a deterministic generator and can
// optionally use an OpenAI-compatible LLM when an API key is configured.
package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/srl-labs/containerlab/studio/engine"
	"github.com/srl-labs/containerlab/studio/model"
)

// Config configures the Copilot agent.
type Config struct {
	BaseURL string
	Model   string
	APIKey  string
	Engine  engine.Engine
}

// Agent is the ClabStudio Copilot.
type Agent struct {
	cfg      Config
	provider *Provider
}

// NewAgent builds a Copilot agent. When an API key is present a live LLM
// provider is attached; otherwise the agent runs in offline (deterministic)
// mode.
func NewAgent(cfg Config) *Agent {
	a := &Agent{cfg: cfg}

	if strings.TrimSpace(cfg.APIKey) != "" {
		a.provider = NewProvider(cfg.BaseURL, cfg.Model, cfg.APIKey)
	}

	return a
}

// Available reports whether a live LLM backend is configured. Even when false,
// the agent can still generate topologies via the offline generator.
func (a *Agent) Available() bool {
	return a.provider != nil
}

// ChatRequest is a Copilot chat turn.
type ChatRequest struct {
	Message string `json:"message"`
	// Lab is the currently open lab name (used to name generated topologies).
	Lab string `json:"lab,omitempty"`
}

// ChatReply is the Copilot response for a chat turn.
type ChatReply struct {
	// Reply is the assistant's natural-language message.
	Reply string `json:"reply"`
	// ProposedGraph, when present, is a topology the user can apply/deploy.
	ProposedGraph *model.Graph `json:"proposedGraph,omitempty"`
	// Notes are additional bullet points about the proposal.
	Notes []string `json:"notes,omitempty"`
	// Source indicates how the reply was produced: "llm" or "offline".
	Source string `json:"source"`
}

// Chat handles a single Copilot turn. Topology requests produce a proposed
// graph (via the LLM when configured, otherwise the deterministic generator).
// Non-topology messages are answered by the LLM when available.
func (a *Agent) Chat(ctx context.Context, req ChatRequest) (*ChatReply, error) {
	msg := strings.TrimSpace(req.Message)
	if msg == "" {
		return &ChatReply{Reply: "Tell me what network you'd like to build.", Source: "offline"}, nil
	}

	if isTopologyRequest(msg) {
		return a.handleTopology(ctx, req)
	}

	// Non-topology conversation.
	if a.Available() {
		text, err := a.provider.Complete(ctx, []ChatMessage{
			{Role: "system", Content: conversationalSystemPrompt},
			{Role: "user", Content: msg},
		})
		if err == nil {
			return &ChatReply{Reply: text, Source: "llm"}, nil
		}
	}

	return &ChatReply{
		Reply: "I'm the ClabStudio Copilot. Describe a network and I'll build it — " +
			"for example: \"3-node OSPF triangle with SR Linux\", \"leaf-spine fabric with 3 leaves and 2 spines using Arista\", " +
			"or \"4 FRR routers in a ring, each with a Linux host\".",
		Source: "offline",
	}, nil
}

// handleTopology produces a proposed topology graph.
func (a *Agent) handleTopology(ctx context.Context, req ChatRequest) (*ChatReply, error) {
	// Try the LLM first when configured; fall back to the offline generator.
	if a.Available() {
		if g, err := a.llmTopology(ctx, req); err == nil && g != nil && len(g.Nodes) > 0 {
			return &ChatReply{
				Reply: fmt.Sprintf("Designed a topology with %d nodes and %d links. "+
					"Review it on the canvas, then Apply and Deploy.", len(g.Nodes), len(g.Links)),
				ProposedGraph: g,
				Source:        "llm",
			}, nil
		}
	}

	res, err := Generate(req.Message, req.Lab)
	if err != nil {
		return nil, err
	}

	return &ChatReply{
		Reply:         res.Summary,
		ProposedGraph: res.Graph,
		Notes:         res.Notes,
		Source:        "offline",
	}, nil
}

// llmTopology asks the LLM to return a topology as JSON matching the graph model.
func (a *Agent) llmTopology(ctx context.Context, req ChatRequest) (*model.Graph, error) {
	labName := req.Lab
	if labName == "" {
		labName = "ai-lab"
	}

	text, err := a.provider.Complete(ctx, []ChatMessage{
		{Role: "system", Content: topologySystemPrompt},
		{Role: "user", Content: fmt.Sprintf("Lab name: %s\nRequest: %s", labName, req.Message)},
	})
	if err != nil {
		return nil, err
	}

	g, err := parseGraphJSON(text)
	if err != nil {
		return nil, err
	}

	if g.Name == "" {
		g.Name = labName
	}

	return g, nil
}

// parseGraphJSON extracts a model.Graph from an LLM response, tolerating code
// fences and surrounding prose.
func parseGraphJSON(text string) (*model.Graph, error) {
	s := strings.TrimSpace(text)
	s = strings.TrimPrefix(s, "```json")
	s = strings.TrimPrefix(s, "```")
	s = strings.TrimSuffix(s, "```")

	start := strings.Index(s, "{")
	end := strings.LastIndex(s, "}")

	if start < 0 || end <= start {
		return nil, fmt.Errorf("no JSON object found in LLM response")
	}

	var g model.Graph
	if err := json.Unmarshal([]byte(s[start:end+1]), &g); err != nil {
		return nil, err
	}

	if g.Nodes == nil {
		g.Nodes = []*model.Node{}
	}

	if g.Links == nil {
		g.Links = []*model.Link{}
	}

	return &g, nil
}

// isTopologyRequest heuristically decides whether a message asks to build a lab.
func isTopologyRequest(msg string) bool {
	p := strings.ToLower(msg)

	keywords := []string{
		"topolog", "lab", "build", "create", "generate", "design", "make",
		"router", "switch", "node", "leaf", "spine", "mesh", "ring", "star",
		"clos", "fabric", "network", "srlinux", "arista", "frr", "juniper",
		"ospf", "bgp", "isis", "vxlan", "evpn", "host", "linear", "chain",
	}

	for _, k := range keywords {
		if strings.Contains(p, k) {
			return true
		}
	}

	return false
}

const conversationalSystemPrompt = "You are the ClabStudio Copilot, an expert network engineer " +
	"assistant embedded in a containerlab-based network simulator. Answer concisely and helpfully. " +
	"If the user seems to want a lab built, encourage them to describe the topology."

const topologySystemPrompt = `You are the ClabStudio Copilot. Convert the user's request into a
containerlab topology and respond with ONLY a JSON object (no prose, no code fences) matching this schema:
{
  "name": string,
  "nodes": [ { "name": string, "kind": string, "image": string, "position": {"x": number, "y": number} } ],
  "links": [ { "source": string, "sourceEndpoint": string, "target": string, "targetEndpoint": string } ]
}
Rules:
- kind must be one of: linux, nokia_srlinux, arista_ceos, juniper_crpd, sonic-vs, cvx.
- Use nokia_srlinux interface names like e1-1, e1-2; use eth1, eth2 for linux and others.
- Give every node a distinct position on a grid (x,y in pixels, ~200px apart).
- Every link connects two existing node names via distinct endpoints.
- Keep it to at most 24 nodes.`

