// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"errors"
	"testing"

	"github.com/srl-labs/containerlab/studio/model"
)

func sampleGraph(name string) *model.Graph {
	return &model.Graph{
		Name: name,
		Nodes: []*model.Node{
			{Name: "n1", Kind: "linux", Image: "alpine"},
			{Name: "n2", Kind: "linux", Image: "alpine"},
		},
		Links: []*model.Link{
			{Source: "n1", SourceEndpoint: "eth1", Target: "n2", TargetEndpoint: "eth1"},
		},
	}
}

func TestFakeEngineLifecycle(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)

	if err := e.SaveLab(ctx, sampleGraph("lab1")); err != nil {
		t.Fatalf("save: %v", err)
	}

	labs, err := e.ListLabs(ctx)
	if err != nil {
		t.Fatalf("list: %v", err)
	}

	if len(labs) != 1 || labs[0].Name != "lab1" || labs[0].NodeCount != 2 {
		t.Fatalf("unexpected labs: %+v", labs)
	}

	if labs[0].Deployed {
		t.Fatal("lab should not be deployed yet")
	}

	st, err := e.Deploy(ctx, "lab1")
	if err != nil {
		t.Fatalf("deploy: %v", err)
	}

	if !st.Deployed || len(st.Nodes) != 2 {
		t.Fatalf("unexpected status: %+v", st)
	}

	res, err := e.Exec(ctx, "lab1", "n1", "echo hi")
	if err != nil {
		t.Fatalf("exec: %v", err)
	}

	if res.ReturnCode != 0 || res.Stdout == "" {
		t.Fatalf("unexpected exec result: %+v", res)
	}

	if err := e.Destroy(ctx, "lab1", false); err != nil {
		t.Fatalf("destroy: %v", err)
	}

	yaml, err := e.RenderYAML(ctx, "lab1")
	if err != nil {
		t.Fatalf("render: %v", err)
	}

	if len(yaml) == 0 {
		t.Fatal("expected non-empty yaml")
	}
}

func TestFakeEngineRuntimeDown(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(false)

	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	if _, err := e.Deploy(ctx, "lab1"); !errors.Is(err, ErrRuntimeUnavailable) {
		t.Fatalf("expected ErrRuntimeUnavailable, got %v", err)
	}

	cap := e.Capabilities(ctx)
	if cap.RuntimeAvailable {
		t.Fatal("expected runtime unavailable")
	}
}

func TestFakeEngineDeleteGuards(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)

	_ = e.SaveLab(ctx, sampleGraph("lab1"))
	_, _ = e.Deploy(ctx, "lab1")

	if err := e.DeleteLab(ctx, "lab1"); err == nil {
		t.Fatal("expected error deleting a deployed lab")
	}

	_ = e.Destroy(ctx, "lab1", false)

	if err := e.DeleteLab(ctx, "lab1"); err != nil {
		t.Fatalf("delete after destroy: %v", err)
	}

	if err := e.DeleteLab(ctx, "missing"); err == nil {
		t.Fatal("expected error deleting missing lab")
	}
}
