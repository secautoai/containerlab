package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeTopo(t *testing.T, yml string) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "topo.clab.yml")
	if err := os.WriteFile(p, []byte(yml), 0o644); err != nil {
		t.Fatal(err)
	}
	return p
}

func TestParseExample(t *testing.T) {
	lab, err := Parse("examples/pair.clab.yml")
	if err != nil {
		t.Fatal(err)
	}
	if lab.Name != "pair" || len(lab.Topology.Nodes) != 2 || len(lab.Topology.Links) != 1 {
		t.Fatalf("unexpected model: %+v", lab)
	}
	if got := lab.NodeNames(); got[0] != "n1" || got[1] != "n2" {
		t.Fatalf("NodeNames = %v", got)
	}
	if img := lab.Topology.Nodes["n1"].Image; img != "minilab/nodeagent:v1" {
		t.Fatalf("n1 image = %q", img)
	}
	if ex := lab.Topology.Nodes["n2"].Exec; len(ex) != 1 || ex[0] != "/nodeagent -serve :9000" {
		t.Fatalf("n2 exec = %v", ex)
	}
	ep := lab.Topology.Links[0].EP(1)
	if ep != (Endpoint{Node: "n2", Iface: "eth1", IPv4: "10.10.0.2/24"}) {
		t.Fatalf("endpoint 1 = %+v", ep)
	}
	got := lab.IfacesFor("n1")
	if len(got) != 1 || got[0] != (Endpoint{Node: "n1", Iface: "eth1", IPv4: "10.10.0.1/24"}) {
		t.Fatalf("IfacesFor(n1) = %+v", got)
	}
}

// mk wraps a links snippet in a two-node topology.
func mk(links string) string {
	return "name: t\ntopology:\n  nodes:\n    n1: {image: img}\n    n2: {image: img}\n  links:\n" + links
}

func TestValidateErrors(t *testing.T) {
	cases := []struct{ name, yml, want string }{
		{"unknown node", mk("    - endpoints: [\"n1:eth1\", \"nX:eth1\"]\n"), "unknown node"},
		{"dup endpoint across links", mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\"]\n    - endpoints: [\"n1:eth1\", \"n2:eth2\"]\n"), "duplicate endpoint"},
		{"dup endpoint within link", mk("    - endpoints: [\"n1:eth1\", \"n1:eth1\"]\n"), "duplicate endpoint"},
		{"bare node", mk("    - endpoints: [\"n1\", \"n2:eth1\"]\n"), `want "<node>:<iface>"`},
		{"empty iface", mk("    - endpoints: [\"n1:\", \"n2:eth1\"]\n"), `want "<node>:<iface>"`},
		{"extra colon", mk("    - endpoints: [\"n1:e:x\", \"n2:eth1\"]\n"), `want "<node>:<iface>"`},
		{"one endpoint", mk("    - endpoints: [\"n1:eth1\"]\n"), "exactly 2 endpoints"},
		{"three endpoints", mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\", \"n2:eth2\"]\n"), "exactly 2 endpoints"},
		{"ipv4 longer than endpoints", mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\"]\n      ipv4: [\"10.0.0.1/24\", \"10.0.0.2/24\", \"10.0.0.3/24\"]\n"), "ipv4 entries"},
		{"ipv4 missing mask", mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\"]\n      ipv4: [\"10.0.0.1\"]\n"), "not an IPv4 CIDR"},
		{"ipv4 is v6", mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\"]\n      ipv4: [\"2001:db8::1/64\"]\n"), "not an IPv4 CIDR"},
		{"missing image", "name: t\ntopology:\n  nodes:\n    n1: {}\n", "image is required"},
		{"null node", "name: t\ntopology:\n  nodes:\n    n1:\n", "image is required"},
		{"missing name", "topology:\n  nodes:\n    n1: {image: img}\n", "name is required"},
		{"no nodes", "name: t\ntopology: {}\n", "no nodes"},
		{"dup node name", "name: t\ntopology:\n  nodes:\n    n1: {image: a}\n    n1: {image: b}\n", "already defined"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			_, err := Parse(writeTopo(t, c.yml))
			if err == nil || !strings.Contains(err.Error(), c.want) {
				t.Fatalf("want error containing %q, got: %v", c.want, err)
			}
		})
	}
}

func TestIPv4SkipAndShort(t *testing.T) {
	yml := mk("    - endpoints: [\"n1:eth1\", \"n2:eth1\"]\n      ipv4: [\"\", \"10.0.0.2/24\"]\n" +
		"    - endpoints: [\"n1:eth2\", \"n2:eth2\"]\n      ipv4: [\"10.0.1.1/30\"]\n")
	lab, err := Parse(writeTopo(t, yml))
	if err != nil {
		t.Fatal(err)
	}
	l0, l1 := lab.Topology.Links[0], lab.Topology.Links[1]
	if l0.EP(0).IPv4 != "" || l0.EP(1).IPv4 != "10.0.0.2/24" {
		t.Fatalf("link 0 addrs: %+v %+v", l0.EP(0), l0.EP(1))
	}
	if l1.EP(0).IPv4 != "10.0.1.1/30" || l1.EP(1).IPv4 != "" {
		t.Fatalf("link 1 addrs: %+v %+v", l1.EP(0), l1.EP(1))
	}
}
