// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"strings"
	"testing"

	"github.com/srl-labs/containerlab/studio/model"
)

func TestExplainGraph(t *testing.T) {
	g := &model.Graph{
		Name: "demo",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "nokia_srlinux", Image: "srl"},
			{Name: "r2", Kind: "nokia_srlinux", Image: "srl"},
			{Name: "h1", Kind: "linux", Image: "alpine"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "e1-1", Target: "r2", TargetEndpoint: "e1-1"},
		},
	}

	s := ExplainGraph(g)
	if !strings.Contains(s, "3 node") || !strings.Contains(s, "1 link") {
		t.Errorf("unexpected explanation: %q", s)
	}

	if !strings.Contains(s, "No IP addressing") {
		t.Errorf("expected addressing note, got: %q", s)
	}
}

func TestExplainGraphEmpty(t *testing.T) {
	if !strings.Contains(ExplainGraph(&model.Graph{}), "empty") {
		t.Error("expected empty note")
	}
}

func TestTroubleshootGraph(t *testing.T) {
	g := &model.Graph{
		Name: "bad",
		Nodes: []*model.Node{
			{Name: "c", Kind: "nokia_srlinux"}, // no image
		},
		Links: []*model.Link{
			{Source: "c", SourceEndpoint: "e1-1", Target: "ghost", TargetEndpoint: "e1-1"}, // dangling
		},
	}

	summary, notes := TroubleshootGraph(g)
	if !strings.Contains(summary, "error") {
		t.Errorf("unexpected summary: %q", summary)
	}

	joined := strings.Join(notes, "\n")
	if !strings.Contains(joined, "remove the link or add the referenced node") {
		t.Errorf("expected dangling-link suggestion, got: %v", notes)
	}
}

func TestTroubleshootClean(t *testing.T) {
	g := &model.Graph{
		Name: "ok",
		Nodes: []*model.Node{
			{Name: "a", Kind: "linux", Image: "alpine"},
			{Name: "b", Kind: "linux", Image: "alpine"},
		},
		Links: []*model.Link{
			{Source: "a", SourceEndpoint: "eth1", Target: "b", TargetEndpoint: "eth1"},
		},
	}

	summary, notes := TroubleshootGraph(g)
	if !strings.Contains(summary, "No design issues") || len(notes) != 0 {
		t.Errorf("expected clean troubleshoot, got %q / %v", summary, notes)
	}
}

func TestInsightIntents(t *testing.T) {
	if !explainIntent("explain this lab") || !explainIntent("give me an overview") {
		t.Error("expected explain intent")
	}

	if !troubleshootIntent("what's wrong with my lab?") || !troubleshootIntent("troubleshoot") {
		t.Error("expected troubleshoot intent")
	}

	// phrasing variants that previously fell through to topology generation
	for _, m := range []string{"what is wrong with my lab?", "my lab is broken", "any problem here?"} {
		if !troubleshootIntent(m) {
			t.Errorf("expected troubleshoot intent for %q", m)
		}
	}

	if explainIntent("add a node") || troubleshootIntent("build a ring") {
		t.Error("false positive intent")
	}
}
