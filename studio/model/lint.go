// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import "strings"

// Severity levels for lint issues.
const (
	SeverityError   = "error"
	SeverityWarning = "warning"
)

// LintIssue is a single problem found while validating a topology graph.
type LintIssue struct {
	Severity string `json:"severity"`
	Code     string `json:"code"`
	Message  string `json:"message"`
	Node     string `json:"node,omitempty"`
}

// LintResult aggregates lint issues with counts.
type LintResult struct {
	Issues   []LintIssue `json:"issues"`
	Errors   int         `json:"errors"`
	Warnings int         `json:"warnings"`
	OK       bool        `json:"ok"`
}

// containerKinds returns the set of catalog kinds that run as containers and so
// require an image.
func containerKinds() map[string]bool {
	out := map[string]bool{}
	for _, k := range Catalog() {
		if k.Container {
			out[k.Kind] = true
		}
	}

	return out
}

// Lint validates a topology graph and returns any issues found. It is a pure
// function performing design-time (pre-deploy) checks.
func Lint(g *Graph) *LintResult {
	res := &LintResult{Issues: []LintIssue{}}

	add := func(sev, code, msg, node string) {
		res.Issues = append(res.Issues, LintIssue{Severity: sev, Code: code, Message: msg, Node: node})
	}

	if g == nil || len(g.Nodes) == 0 {
		add(SeverityWarning, "EMPTY", "topology has no nodes", "")
		finalizeLint(res)

		return res
	}

	containers := containerKinds()

	// Node checks + name index.
	nameCount := map[string]int{}
	for _, n := range g.Nodes {
		nameCount[n.Name]++
	}

	nodeExists := map[string]bool{}

	for _, n := range g.Nodes {
		nodeExists[n.Name] = true

		if strings.TrimSpace(n.Name) == "" {
			add(SeverityError, "BAD_NAME", "node has an empty name", n.Name)
		} else if strings.ContainsAny(n.Name, " \t/") {
			add(SeverityWarning, "BAD_NAME",
				"node name contains spaces or slashes which may cause issues", n.Name)
		}

		if nameCount[n.Name] > 1 {
			add(SeverityError, "DUP_NODE", "duplicate node name", n.Name)
		}

		if strings.TrimSpace(n.Kind) == "" {
			add(SeverityError, "NO_KIND", "node has no kind", n.Name)
		}

		if containers[n.Kind] && strings.TrimSpace(n.Image) == "" {
			add(SeverityWarning, "NO_IMAGE",
				"node has no image; deployment will fail unless the kind has a default", n.Name)
		}
	}

	// Link checks + per-node interface usage + connectivity set.
	ifaceUsed := map[string]bool{} // key: node/iface
	connected := map[string]bool{}

	for _, l := range g.Links {
		if l == nil {
			continue
		}

		if l.Source == l.Target && l.Source != "" {
			add(SeverityError, "SELF_LOOP", "link connects a node to itself", l.Source)
			continue
		}

		for _, ep := range []struct{ node, iface string }{
			{l.Source, l.SourceEndpoint},
			{l.Target, l.TargetEndpoint},
		} {
			if ep.node == "" {
				continue
			}

			connected[ep.node] = true

			if !nodeExists[ep.node] {
				add(SeverityError, "DANGLING_LINK",
					"link references unknown node "+ep.node, ep.node)

				continue
			}

			if ep.iface != "" {
				key := ep.node + "/" + ep.iface
				if ifaceUsed[key] {
					add(SeverityError, "IFACE_CONFLICT",
						"interface "+ep.iface+" is used by more than one link", ep.node)
				}

				ifaceUsed[key] = true
			}
		}
	}

	// Isolated nodes (only meaningful when there is more than one node).
	if len(g.Nodes) > 1 {
		for _, n := range g.Nodes {
			if !connected[n.Name] {
				add(SeverityWarning, "ISOLATED", "node has no links", n.Name)
			}
		}
	}

	finalizeLint(res)

	return res
}

func finalizeLint(res *LintResult) {
	for _, i := range res.Issues {
		switch i.Severity {
		case SeverityError:
			res.Errors++
		case SeverityWarning:
			res.Warnings++
		}
	}

	res.OK = res.Errors == 0
}
