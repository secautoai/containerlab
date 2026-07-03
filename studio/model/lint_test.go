// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import "testing"

func hasCode(res *LintResult, code string) bool {
	for _, i := range res.Issues {
		if i.Code == code {
			return true
		}
	}

	return false
}

func TestLintClean(t *testing.T) {
	g := &Graph{
		Name: "ok",
		Nodes: []*Node{
			{Name: "a", Kind: "linux", Image: "alpine"},
			{Name: "b", Kind: "linux", Image: "alpine"},
		},
		Links: []*Link{
			{Source: "a", SourceEndpoint: "eth1", Target: "b", TargetEndpoint: "eth1"},
		},
	}

	res := Lint(g)
	if !res.OK || res.Errors != 0 {
		t.Fatalf("expected clean lint, got %+v", res)
	}
}

func TestLintEmpty(t *testing.T) {
	res := Lint(&Graph{Name: "x"})
	if !hasCode(res, "EMPTY") {
		t.Errorf("expected EMPTY warning")
	}
}

func TestLintDetectsProblems(t *testing.T) {
	g := &Graph{
		Name: "bad",
		Nodes: []*Node{
			{Name: "a", Kind: "linux", Image: "alpine"},
			{Name: "a", Kind: "linux", Image: "alpine"},   // duplicate
			{Name: "b", Kind: ""},                         // no kind
			{Name: "c", Kind: "nokia_srlinux"},            // container, no image
			{Name: "iso", Kind: "linux", Image: "alpine"}, // isolated
		},
		Links: []*Link{
			{Source: "a", SourceEndpoint: "eth1", Target: "b", TargetEndpoint: "eth1"},
			{Source: "a", SourceEndpoint: "eth1", Target: "c", TargetEndpoint: "e1-1"}, // iface conflict on a
			{Source: "a", Target: "a"}, // self-loop
			{Source: "a", SourceEndpoint: "eth9", Target: "ghost", TargetEndpoint: "eth1"}, // dangling
		},
	}

	res := Lint(g)

	for _, code := range []string{"DUP_NODE", "NO_KIND", "NO_IMAGE", "IFACE_CONFLICT", "SELF_LOOP", "DANGLING_LINK", "ISOLATED"} {
		if !hasCode(res, code) {
			t.Errorf("expected lint code %q, issues=%+v", code, res.Issues)
		}
	}

	if res.OK {
		t.Errorf("expected lint to fail (errors present)")
	}

	if res.Errors == 0 {
		t.Errorf("expected error count > 0")
	}
}

func TestLintBadName(t *testing.T) {
	g := &Graph{
		Name:  "n",
		Nodes: []*Node{{Name: "bad name", Kind: "linux", Image: "alpine"}},
	}

	res := Lint(g)
	if !hasCode(res, "BAD_NAME") {
		t.Errorf("expected BAD_NAME warning")
	}
}
