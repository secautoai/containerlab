// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package api

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/cookiejar"
	"net/http/httptest"
	"testing"

	"github.com/srl-labs/containerlab/studio/ai"
	"github.com/srl-labs/containerlab/studio/engine"
	"github.com/srl-labs/containerlab/studio/model"
)

func newTestServer(runtimeUp bool) (*httptest.Server, *engine.FakeEngine) {
	eng := engine.NewFakeEngine(runtimeUp)
	agent := ai.NewAgent(ai.Config{Engine: eng}) // offline agent (no API key)
	h := New(eng, agent, "")

	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	return httptest.NewServer(mux), eng
}

// newAuthTestServer returns a server with authentication enabled.
func newAuthTestServer(token string) *httptest.Server {
	eng := engine.NewFakeEngine(true)
	agent := ai.NewAgent(ai.Config{Engine: eng})
	h := New(eng, agent, token)

	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	return httptest.NewServer(h.AuthMiddleware(mux))
}

func TestHealthAndCatalog(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	resp, err := http.Get(srv.URL + "/api/health")
	if err != nil {
		t.Fatalf("health: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("health status: %d", resp.StatusCode)
	}

	resp, err = http.Get(srv.URL + "/api/catalog")
	if err != nil {
		t.Fatalf("catalog: %v", err)
	}

	var kinds []model.KindInfo
	if err := json.NewDecoder(resp.Body).Decode(&kinds); err != nil {
		t.Fatalf("decode catalog: %v", err)
	}

	if len(kinds) == 0 {
		t.Fatal("expected non-empty catalog")
	}
}

func TestLabCRUDFlow(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	// create
	body, _ := json.Marshal(createLabRequest{Name: "lab1"})
	resp, err := http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatalf("create: %v", err)
	}

	if resp.StatusCode != http.StatusCreated {
		t.Fatalf("create status: %d", resp.StatusCode)
	}

	// save (add nodes + link)
	g := &model.Graph{
		Name: "lab1",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "linux", Image: "alpine", Position: model.Position{X: 1, Y: 2}},
			{Name: "r2", Kind: "linux", Image: "alpine"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "eth1", Target: "r2", TargetEndpoint: "eth1"},
		},
	}
	gb, _ := json.Marshal(g)

	req, _ := http.NewRequest(http.MethodPut, srv.URL+"/api/labs/lab1", bytes.NewReader(gb))
	req.Header.Set("Content-Type", "application/json")

	resp, err = http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("save: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("save status: %d", resp.StatusCode)
	}

	// get
	resp, err = http.Get(srv.URL + "/api/labs/lab1")
	if err != nil {
		t.Fatalf("get: %v", err)
	}

	var got model.Graph
	if err := json.NewDecoder(resp.Body).Decode(&got); err != nil {
		t.Fatalf("decode get: %v", err)
	}

	if len(got.Nodes) != 2 || len(got.Links) != 1 {
		t.Fatalf("unexpected graph: %d nodes %d links", len(got.Nodes), len(got.Links))
	}

	// deploy
	resp, err = http.Post(srv.URL+"/api/labs/lab1/deploy", "application/json", nil)
	if err != nil {
		t.Fatalf("deploy: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("deploy status: %d", resp.StatusCode)
	}

	// exec
	eb, _ := json.Marshal(execRequest{Cmd: "uname -a"})
	resp, err = http.Post(srv.URL+"/api/labs/lab1/nodes/r1/exec", "application/json", bytes.NewReader(eb))
	if err != nil {
		t.Fatalf("exec: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("exec status: %d", resp.StatusCode)
	}

	// destroy
	resp, err = http.Post(srv.URL+"/api/labs/lab1/destroy", "application/json", nil)
	if err != nil {
		t.Fatalf("destroy: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("destroy status: %d", resp.StatusCode)
	}

	// delete
	req, _ = http.NewRequest(http.MethodDelete, srv.URL+"/api/labs/lab1", nil)

	resp, err = http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("delete: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("delete status: %d", resp.StatusCode)
	}
}

func TestDeployRuntimeUnavailable(t *testing.T) {
	srv, _ := newTestServer(false)
	defer srv.Close()

	body, _ := json.Marshal(createLabRequest{Name: "lab1"})
	_, _ = http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))

	resp, err := http.Post(srv.URL+"/api/labs/lab1/deploy", "application/json", nil)
	if err != nil {
		t.Fatalf("deploy: %v", err)
	}

	if resp.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d", resp.StatusCode)
	}
}

func TestAIChatOffline(t *testing.T) {
	srv, _ := newTestServer(false)
	defer srv.Close()

	body, _ := json.Marshal(map[string]string{"message": "build a 3 node linear srlinux lab"})

	resp, err := http.Post(srv.URL+"/api/ai/chat", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatalf("ai chat: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("ai chat status: %d", resp.StatusCode)
	}

	var reply ai.ChatReply
	if err := json.NewDecoder(resp.Body).Decode(&reply); err != nil {
		t.Fatalf("decode reply: %v", err)
	}

	if reply.ProposedGraph == nil || len(reply.ProposedGraph.Nodes) != 3 {
		t.Fatalf("expected proposed graph with 3 nodes, got %+v", reply.ProposedGraph)
	}

	if reply.Source != "offline" {
		t.Errorf("expected offline source, got %q", reply.Source)
	}
}

func TestUpdateLabYAML(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	body, _ := json.Marshal(createLabRequest{Name: "y1"})
	_, _ = http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))

	// valid YAML update
	yaml := `name: ignored
topology:
  nodes:
    x: {kind: linux, image: alpine}
    y: {kind: linux, image: alpine}
  links:
    - endpoints: ["x:eth1", "y:eth1"]
`
	resp, err := http.Post(srv.URL+"/api/labs/y1/yaml", "application/x-yaml", bytes.NewReader([]byte(yaml)))
	if err != nil {
		t.Fatalf("update yaml: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("update yaml status: %d", resp.StatusCode)
	}

	var g model.Graph
	_ = json.NewDecoder(resp.Body).Decode(&g)

	// name comes from the path, not the body
	if g.Name != "y1" || len(g.Nodes) != 2 || len(g.Links) != 1 {
		t.Fatalf("unexpected graph after yaml update: %+v", g)
	}

	for _, n := range g.Nodes {
		if n.Position.X == 0 && n.Position.Y == 0 {
			t.Errorf("node %s missing auto-layout position", n.Name)
		}
	}

	// invalid YAML => 400
	resp, err = http.Post(srv.URL+"/api/labs/y1/yaml", "application/x-yaml",
		bytes.NewReader([]byte(":\n  bad: [unterminated")))
	if err != nil {
		t.Fatalf("invalid yaml: %v", err)
	}

	if resp.StatusCode != http.StatusBadRequest {
		t.Fatalf("expected 400 for invalid yaml, got %d", resp.StatusCode)
	}
}

func TestImportLab(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	yaml := `name: imported
topology:
  nodes:
    a: {kind: linux, image: alpine}
    b: {kind: linux, image: alpine}
    c: {kind: linux, image: alpine}
  links:
    - endpoints: ["a:eth1", "b:eth1"]
    - endpoints: ["b:eth2", "c:eth1"]
`
	body, _ := json.Marshal(importRequest{YAML: yaml})

	resp, err := http.Post(srv.URL+"/api/labs/import", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatalf("import: %v", err)
	}

	if resp.StatusCode != http.StatusCreated {
		t.Fatalf("import status: %d", resp.StatusCode)
	}

	var g model.Graph
	if err := json.NewDecoder(resp.Body).Decode(&g); err != nil {
		t.Fatalf("decode: %v", err)
	}

	if g.Name != "imported" || len(g.Nodes) != 3 || len(g.Links) != 2 {
		t.Fatalf("unexpected imported graph: %+v", g)
	}

	// auto-layout should give every node a non-zero position
	for _, n := range g.Nodes {
		if n.Position.X == 0 && n.Position.Y == 0 {
			t.Errorf("node %s missing auto-layout position", n.Name)
		}
	}
}

func TestSaveConfigsRuntimeDown(t *testing.T) {
	srv, _ := newTestServer(false)
	defer srv.Close()

	body, _ := json.Marshal(createLabRequest{Name: "s1"})
	_, _ = http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))

	resp, err := http.Post(srv.URL+"/api/labs/s1/save", "application/json", nil)
	if err != nil {
		t.Fatalf("save: %v", err)
	}

	if resp.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d", resp.StatusCode)
	}
}

func TestCloneAndRenameLab(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	body, _ := json.Marshal(createLabRequest{Name: "orig"})
	_, _ = http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))

	// clone
	cb, _ := json.Marshal(nameRequest{Name: "copy"})
	resp, err := http.Post(srv.URL+"/api/labs/orig/clone", "application/json", bytes.NewReader(cb))
	if err != nil {
		t.Fatalf("clone: %v", err)
	}

	if resp.StatusCode != http.StatusCreated {
		t.Fatalf("clone status: %d", resp.StatusCode)
	}

	// rename
	rb, _ := json.Marshal(nameRequest{Name: "renamed"})
	resp, err = http.Post(srv.URL+"/api/labs/copy/rename", "application/json", bytes.NewReader(rb))
	if err != nil {
		t.Fatalf("rename: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("rename status: %d", resp.StatusCode)
	}

	// verify: orig + renamed exist, copy gone
	resp, _ = http.Get(srv.URL + "/api/labs")

	var labs []engine.LabSummary
	_ = json.NewDecoder(resp.Body).Decode(&labs)

	names := map[string]bool{}
	for _, l := range labs {
		names[l.Name] = true
	}

	if !names["orig"] || !names["renamed"] || names["copy"] {
		t.Fatalf("unexpected labs after clone/rename: %+v", names)
	}
}

func TestConfigureLab(t *testing.T) {
	srv, _ := newTestServer(true)
	defer srv.Close()

	// create + save a 2-linux-node lab
	body, _ := json.Marshal(createLabRequest{Name: "cfg"})
	_, _ = http.Post(srv.URL+"/api/labs", "application/json", bytes.NewReader(body))

	g := &model.Graph{
		Name: "cfg",
		Nodes: []*model.Node{
			{Name: "r1", Kind: "linux", Image: "frr"},
			{Name: "r2", Kind: "linux", Image: "frr"},
		},
		Links: []*model.Link{
			{Source: "r1", SourceEndpoint: "eth1", Target: "r2", TargetEndpoint: "eth1"},
		},
	}
	gb, _ := json.Marshal(g)
	req, _ := http.NewRequest(http.MethodPut, srv.URL+"/api/labs/cfg", bytes.NewReader(gb))
	req.Header.Set("Content-Type", "application/json")
	_, _ = http.DefaultClient.Do(req)

	// configure with OSPF
	cb, _ := json.Marshal(configureRequest{Protocol: "ospf"})
	resp, err := http.Post(srv.URL+"/api/labs/cfg/configure", "application/json", bytes.NewReader(cb))
	if err != nil {
		t.Fatalf("configure: %v", err)
	}

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("configure status: %d", resp.StatusCode)
	}

	var out configureResponse
	if err := json.NewDecoder(resp.Body).Decode(&out); err != nil {
		t.Fatalf("decode: %v", err)
	}

	if out.Plan == nil || len(out.Plan.Nodes) != 2 {
		t.Fatalf("expected addressing plan for 2 nodes, got %+v", out.Plan)
	}

	// each linux node should now have exec commands persisted
	for _, n := range out.Graph.Nodes {
		if len(n.Exec) == 0 {
			t.Fatalf("node %s missing exec after configure", n.Name)
		}
	}
}

func TestAuthFlow(t *testing.T) {
	srv := newAuthTestServer("s3cret")
	defer srv.Close()

	// health + capabilities are exempt
	if resp, _ := http.Get(srv.URL + "/api/health"); resp.StatusCode != http.StatusOK {
		t.Fatalf("health should be exempt, got %d", resp.StatusCode)
	}

	// protected route without auth => 401
	resp, err := http.Get(srv.URL + "/api/labs")
	if err != nil {
		t.Fatalf("labs: %v", err)
	}

	if resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("expected 401 without auth, got %d", resp.StatusCode)
	}

	// wrong token login => 401
	bad, _ := json.Marshal(loginRequest{Token: "nope"})
	resp, _ = http.Post(srv.URL+"/api/login", "application/json", bytes.NewReader(bad))
	if resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("expected 401 for bad token, got %d", resp.StatusCode)
	}

	// correct login => 200 + cookie
	jar, _ := cookiejar.New(nil)
	client := &http.Client{Jar: jar}

	good, _ := json.Marshal(loginRequest{Token: "s3cret"})
	resp, _ = client.Post(srv.URL+"/api/login", "application/json", bytes.NewReader(good))
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200 on login, got %d", resp.StatusCode)
	}

	// now the protected route works with the cookie jar
	resp, _ = client.Get(srv.URL + "/api/labs")
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200 after login, got %d", resp.StatusCode)
	}

	// bearer token also works
	req, _ := http.NewRequest(http.MethodGet, srv.URL+"/api/labs", nil)
	req.Header.Set("Authorization", "Bearer s3cret")

	resp, _ = http.DefaultClient.Do(req)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200 with bearer, got %d", resp.StatusCode)
	}

	// logout clears the cookie => 401 again
	resp, _ = client.Post(srv.URL+"/api/logout", "application/json", nil)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("logout status %d", resp.StatusCode)
	}

	resp, _ = client.Get(srv.URL + "/api/labs")
	if resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("expected 401 after logout, got %d", resp.StatusCode)
	}
}

func TestCapabilities(t *testing.T) {
	srv, _ := newTestServer(false)
	defer srv.Close()

	resp, err := http.Get(srv.URL + "/api/capabilities")
	if err != nil {
		t.Fatalf("capabilities: %v", err)
	}

	var caps map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&caps); err != nil {
		t.Fatalf("decode: %v", err)
	}

	if caps["runtimeAvailable"] != false {
		t.Fatalf("expected runtimeAvailable false, got %v", caps["runtimeAvailable"])
	}
}
