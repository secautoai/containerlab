package main

import (
	"fmt"
	"maps"
	"net/netip"
	"os"
	"slices"
	"strings"

	"gopkg.in/yaml.v3"
)

// Lab is the root of a *.clab.yml file (containerlab-compatible subset).
// Node-name uniqueness is enforced by yaml.v3, which rejects duplicate
// mapping keys.
type Lab struct {
	Name     string `yaml:"name"`
	Topology struct {
		Nodes map[string]*Node `yaml:"nodes"`
		Links []*Link          `yaml:"links"`
	} `yaml:"topology"`
}

// Node declares one container.
type Node struct {
	Image string   `yaml:"image"`
	Exec  []string `yaml:"exec"` // commands run detached after wiring
}

// Link is one point-to-point wire in containerlab brief form:
// endpoints: ["n1:eth1", "n2:eth1"], ipv4: ["10.10.0.1/24", "10.10.0.2/24"].
type Link struct {
	Endpoints []string `yaml:"endpoints"`
	IPv4      []string `yaml:"ipv4"` // positional per endpoint; "" skips one

	eps [2]Endpoint // parsed by Validate
}

// Endpoint is one parsed end of a link.
type Endpoint struct {
	Node  string `json:"node"`
	Iface string `json:"iface"`
	IPv4  string `json:"ipv4,omitempty"` // CIDR or ""
}

// EP returns parsed endpoint i of the link (valid after Validate).
func (ln *Link) EP(i int) Endpoint { return ln.eps[i] }

// Parse reads and validates a topology file.
func Parse(path string) (*Lab, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var lab Lab
	if err := yaml.Unmarshal(b, &lab); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	if err := lab.Validate(); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return &lab, nil
}

// Validate checks the model and fills each link's parsed endpoints.
func (l *Lab) Validate() error {
	if l.Name == "" {
		return fmt.Errorf("lab: name is required")
	}
	if len(l.Topology.Nodes) == 0 {
		return fmt.Errorf("topology: no nodes")
	}
	for name, n := range l.Topology.Nodes {
		if n == nil || n.Image == "" {
			return fmt.Errorf("node %q: image is required", name)
		}
	}
	seen := map[string]bool{} // "<node>:<iface>" must be unique across all links
	for i, ln := range l.Topology.Links {
		if len(ln.Endpoints) != 2 {
			return fmt.Errorf("link %d: want exactly 2 endpoints, got %d", i, len(ln.Endpoints))
		}
		if len(ln.IPv4) > len(ln.Endpoints) {
			return fmt.Errorf("link %d: %d ipv4 entries for %d endpoints", i, len(ln.IPv4), len(ln.Endpoints))
		}
		for j, s := range ln.Endpoints {
			node, iface, ok := strings.Cut(s, ":")
			if !ok || node == "" || iface == "" || strings.Contains(iface, ":") {
				return fmt.Errorf("link %d: endpoint %q: want \"<node>:<iface>\"", i, s)
			}
			if _, known := l.Topology.Nodes[node]; !known {
				return fmt.Errorf("link %d: endpoint %q: unknown node %q", i, s, node)
			}
			if seen[s] {
				return fmt.Errorf("link %d: duplicate endpoint %q", i, s)
			}
			seen[s] = true
			ln.eps[j] = Endpoint{Node: node, Iface: iface}
			if j < len(ln.IPv4) {
				ln.eps[j].IPv4 = ln.IPv4[j]
			}
		}
		for j, a := range ln.IPv4 {
			if a == "" {
				continue
			}
			if p, err := netip.ParsePrefix(a); err != nil || !p.Addr().Is4() {
				return fmt.Errorf("link %d: ipv4[%d] %q: not an IPv4 CIDR", i, j, a)
			}
		}
	}
	return nil
}

// NodeNames returns the node names sorted, for deterministic iteration.
func (l *Lab) NodeNames() []string {
	return slices.Sorted(maps.Keys(l.Topology.Nodes))
}

// IfacesFor returns the declared interfaces (with IPv4s) of one node, in link order.
func (l *Lab) IfacesFor(node string) []Endpoint {
	out := []Endpoint{} // non-nil so inspect --json shows [] not null
	for _, ln := range l.Topology.Links {
		for _, ep := range ln.eps {
			if ep.Node == node {
				out = append(out, ep)
			}
		}
	}
	return out
}
