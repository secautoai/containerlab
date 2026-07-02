// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import "fmt"

// Template is a curated starter topology surfaced in the ClabStudio quick-start
// catalog. Templates are built deterministically by the offline generator, so
// they work without any container runtime or LLM.
type Template struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Category    string `json:"category"`
	// prompt is the internal generator prompt (unexported, not serialized).
	prompt string
}

// templateList is the curated set of starter topologies.
func templateList() []Template {
	return []Template{
		{
			ID:          "linear-3-srl",
			Name:        "3-node chain (SR Linux)",
			Description: "Three Nokia SR Linux routers connected back-to-back.",
			Category:    "Basics",
			prompt:      "3 node linear srlinux lab",
		},
		{
			ID:          "p2p-frr-hosts",
			Name:        "2 routers + hosts (FRR)",
			Description: "Two FRR routers, each with a Linux host attached.",
			Category:    "Basics",
			prompt:      "2 frr routers each with a linux host",
		},
		{
			ID:          "triangle-ospf-srl",
			Name:        "OSPF triangle (SR Linux)",
			Description: "Three SR Linux routers in a triangle — classic OSPF lab.",
			Category:    "Routing",
			prompt:      "ospf triangle srlinux",
		},
		{
			ID:          "ring-4-frr",
			Name:        "4-router ring (FRR)",
			Description: "Four FRR routers in a ring topology.",
			Category:    "Routing",
			prompt:      "4 frr routers in a ring",
		},
		{
			ID:          "mesh-4-srl",
			Name:        "Full mesh (SR Linux)",
			Description: "Four SR Linux routers, fully meshed.",
			Category:    "Routing",
			prompt:      "full mesh of 4 srlinux",
		},
		{
			ID:          "leaf-spine-arista",
			Name:        "Leaf-spine fabric (Arista)",
			Description: "A 2-spine x 2-leaf Clos fabric using Arista cEOS.",
			Category:    "Fabric",
			prompt:      "leaf-spine fabric with 2 leaves and 2 spines arista",
		},
		{
			ID:          "leaf-spine-srl-3x2",
			Name:        "Leaf-spine 3x2 (SR Linux)",
			Description: "A 2-spine x 3-leaf Clos fabric using Nokia SR Linux.",
			Category:    "Fabric",
			prompt:      "leaf-spine fabric with 3 leaves and 2 spines srlinux",
		},
	}
}

// Templates returns the curated quick-start catalog (without internal prompts).
func Templates() []Template {
	return templateList()
}

// BuildTemplate instantiates a template into a topology graph. When name is
// empty a name is derived from the template.
func BuildTemplate(id, name string) (*GenResult, error) {
	for _, t := range templateList() {
		if t.ID == id {
			if name == "" {
				name = id
			}

			return Generate(t.prompt, name)
		}
	}

	return nil, fmt.Errorf("unknown template %q", id)
}
