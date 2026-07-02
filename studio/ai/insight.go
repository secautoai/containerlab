// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"fmt"
	"sort"
	"strings"

	"github.com/srl-labs/containerlab/studio/model"
)

// ExplainGraph returns a plain-English summary of a topology graph.
func ExplainGraph(g *model.Graph) string {
	if g == nil || len(g.Nodes) == 0 {
		return "This lab is empty — add some nodes or ask me to build a topology."
	}

	// Count nodes by kind.
	byKind := map[string]int{}
	for _, n := range g.Nodes {
		byKind[n.Kind]++
	}

	kinds := make([]string, 0, len(byKind))
	for k := range byKind {
		kinds = append(kinds, k)
	}

	sort.Strings(kinds)

	parts := make([]string, 0, len(kinds))
	for _, k := range kinds {
		parts = append(parts, fmt.Sprintf("%d× %s", byKind[k], displayKind(k)))
	}

	var b strings.Builder

	fmt.Fprintf(&b, "Lab %q has %d node(s) (%s) and %d link(s).",
		g.Name, len(g.Nodes), strings.Join(parts, ", "), len(g.Links))

	// Note whether addressing/config has been generated.
	configured := 0

	for _, n := range g.Nodes {
		for _, e := range n.Exec {
			if strings.Contains(e, "ip addr add") {
				configured++
				break
			}
		}
	}

	if configured > 0 {
		fmt.Fprintf(&b, " %d node(s) have generated IP configuration.", configured)
	} else {
		b.WriteString(" No IP addressing has been generated yet — try \"assign IPs\" or \"configure OSPF\".")
	}

	return b.String()
}

// suggestionFor maps a lint code to a human-friendly fix suggestion.
func suggestionFor(code string) string {
	switch code {
	case "NO_IMAGE":
		return "set an image on the node (node drawer or a catalog default)"
	case "NO_KIND":
		return "set a kind for the node"
	case "DANGLING_LINK":
		return "remove the link or add the referenced node"
	case "SELF_LOOP":
		return "remove the self-link"
	case "IFACE_CONFLICT":
		return "assign a different interface to one of the links"
	case "DUP_NODE":
		return "rename one of the duplicate nodes"
	case "ISOLATED":
		return "connect the node to the topology or remove it"
	case "BAD_NAME":
		return "use a name without spaces or slashes"
	case "EMPTY":
		return "add nodes to the topology"
	default:
		return "review the topology"
	}
}

// TroubleshootGraph runs the design linter and returns a summary plus per-issue
// fix suggestions.
func TroubleshootGraph(g *model.Graph) (string, []string) {
	res := model.Lint(g)

	if len(res.Issues) == 0 {
		return "No design issues found. Deploy the lab and run Validate to check reachability.", nil
	}

	summary := fmt.Sprintf("Found %d error(s) and %d warning(s):", res.Errors, res.Warnings)

	notes := make([]string, 0, len(res.Issues))

	seen := map[string]bool{}

	for _, i := range res.Issues {
		prefix := ""
		if i.Node != "" {
			prefix = i.Node + ": "
		}

		notes = append(notes, fmt.Sprintf("%s%s — %s", prefix, i.Message, suggestionFor(i.Code)))
		seen[i.Code] = true
	}

	return summary, notes
}

// explainIntent detects a request to explain/summarize the current lab.
func explainIntent(msg string) bool {
	p := strings.ToLower(msg)

	for _, k := range []string{"explain", "describe", "summarize", "summary", "overview", "what is this", "what's this", "tell me about"} {
		if strings.Contains(p, k) {
			return true
		}
	}

	return false
}

// troubleshootIntent detects a request to check/troubleshoot the current lab.
func troubleshootIntent(msg string) bool {
	p := strings.ToLower(msg)

	for _, k := range []string{
		"wrong", "broken", "not working", "doesn't work", "problem", "issue",
		"troubleshoot", "check the lab", "check my lab", "validate the design", "lint",
	} {
		if strings.Contains(p, k) {
			return true
		}
	}

	return false
}
