// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package ai implements the ClabStudio Copilot: an agent that turns
// plain-English requests into topologies (and, in later milestones, configs and
// validations). It works fully offline via a deterministic generator and can
// optionally use an OpenAI-compatible LLM when an API key is configured.
package ai

import (
	"strings"

	"github.com/srl-labs/containerlab/studio/engine"
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
