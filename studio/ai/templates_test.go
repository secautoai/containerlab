// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import "testing"

func TestTemplatesNonEmpty(t *testing.T) {
	tpls := Templates()
	if len(tpls) == 0 {
		t.Fatal("expected non-empty template catalog")
	}

	for _, tp := range tpls {
		if tp.ID == "" || tp.Name == "" || tp.Category == "" {
			t.Errorf("template missing fields: %+v", tp)
		}
	}
}

func TestBuildEveryTemplate(t *testing.T) {
	for _, tp := range Templates() {
		res, err := BuildTemplate(tp.ID, "")
		if err != nil {
			t.Fatalf("build %q: %v", tp.ID, err)
		}

		if res.Graph == nil || len(res.Graph.Nodes) == 0 {
			t.Fatalf("template %q produced empty graph", tp.ID)
		}

		if res.Graph.Name == "" {
			t.Errorf("template %q produced graph without name", tp.ID)
		}
	}
}

func TestBuildTemplateUnknown(t *testing.T) {
	if _, err := BuildTemplate("does-not-exist", "x"); err == nil {
		t.Fatal("expected error for unknown template")
	}
}

func TestBuildTemplateName(t *testing.T) {
	res, err := BuildTemplate("triangle-ospf-srl", "mylab")
	if err != nil {
		t.Fatalf("build: %v", err)
	}

	if res.Graph.Name != "mylab" {
		t.Errorf("expected name mylab, got %q", res.Graph.Name)
	}
}
