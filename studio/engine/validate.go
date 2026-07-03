// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import "strings"

// ReachabilityCheck is a single node-to-node ping result.
type ReachabilityCheck struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Target string `json:"target"`
	OK     bool   `json:"ok"`
	Detail string `json:"detail,omitempty"`
}

// ValidationReport summarizes a lab's end-to-end reachability.
type ValidationReport struct {
	Lab      string              `json:"lab"`
	Deployed bool                `json:"deployed"`
	Checks   []ReachabilityCheck `json:"checks"`
	Passed   int                 `json:"passed"`
	Failed   int                 `json:"failed"`
	Summary  string              `json:"summary"`
}

// pingSucceeded parses ping stdout to determine whether the target was
// reachable. It recognizes both iproute2/busybox ("0% packet loss" or
// "N received") forms.
func pingSucceeded(output string) bool {
	o := strings.ToLower(output)

	// Check total-loss first: "100% packet loss" contains "0% packet loss".
	if strings.Contains(o, "100% packet loss") || strings.Contains(o, "unreachable") {
		return false
	}

	if strings.Contains(o, "0% packet loss") {
		return true
	}

	// busybox: "2 packets transmitted, 2 packets received, 0% packet loss"
	// treat any non-zero received as success
	if idx := strings.Index(o, "received"); idx >= 0 {
		// look back for the number right before "received"
		prefix := strings.TrimSpace(o[:idx])
		fields := strings.Fields(prefix)
		if len(fields) > 0 {
			last := fields[len(fields)-1]
			if last != "0" && last != "" {
				return true
			}
		}
	}

	// "100% packet loss" or "Network is unreachable" => failure
	return false
}

// summarize builds a human-readable one-liner for a report.
func (r *ValidationReport) summarize() {
	if !r.Deployed {
		r.Summary = "lab is not deployed; nothing to validate"
		return
	}

	total := r.Passed + r.Failed
	if total == 0 {
		r.Summary = "no reachability checks were possible (no management IPs found)"
		return
	}

	if r.Failed == 0 {
		r.Summary = "all reachability checks passed"
		return
	}

	r.Summary = "some reachability checks failed"
}
