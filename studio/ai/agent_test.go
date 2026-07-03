// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"context"
	"testing"

	"github.com/srl-labs/containerlab/studio/engine"
	"github.com/srl-labs/containerlab/studio/model"
)

func TestChatRoutesEditIntent(t *testing.T) {
	ctx := context.Background()
	eng := engine.NewFakeEngine(false)

	_ = eng.SaveLab(ctx, &model.Graph{
		Name: "e",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "nokia_srlinux", Image: "srl"},
			{Name: "r2", Kind: "nokia_srlinux", Image: "srl"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "e1-1", Target: "r2", TargetEndpoint: "e1-1"},
		},
	})

	agent := NewAgent(Config{Engine: eng})

	reply, err := agent.Chat(ctx, ChatRequest{Message: "add a linux host connected to r1", Lab: "e"})
	if err != nil {
		t.Fatalf("chat: %v", err)
	}

	if !reply.Applied {
		t.Fatalf("expected edit to be applied, got reply: %+v", reply)
	}

	if reply.ProposedGraph == nil || len(reply.ProposedGraph.Nodes) != 3 {
		t.Fatalf("expected 3 nodes after edit, got %+v", reply.ProposedGraph)
	}

	// verify persisted
	g, _ := eng.GetLab(ctx, "e")
	if len(g.Nodes) != 3 {
		t.Fatalf("edit not persisted, nodes: %d", len(g.Nodes))
	}
}
