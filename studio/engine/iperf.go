// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"time"
)

// ThroughputResult summarizes an iperf3 run between two nodes.
type ThroughputResult struct {
	From            string  `json:"from"`
	To              string  `json:"to"`
	Target          string  `json:"target"`
	SentBitsPerSec  float64 `json:"sentBitsPerSec"`
	RecvBitsPerSec  float64 `json:"recvBitsPerSec"`
	SentMbitsPerSec float64 `json:"sentMbitsPerSec"`
	RecvMbitsPerSec float64 `json:"recvMbitsPerSec"`
	Retransmits     int     `json:"retransmits"`
	Summary         string  `json:"summary"`
}

// iperf3 --json (abridged) structure.
type iperf3Output struct {
	End struct {
		SumSent struct {
			BitsPerSecond float64 `json:"bits_per_second"`
			Retransmits   int     `json:"retransmits"`
		} `json:"sum_sent"`
		SumReceived struct {
			BitsPerSecond float64 `json:"bits_per_second"`
		} `json:"sum_received"`
	} `json:"end"`
	Error string `json:"error"`
}

// parseIperf3 parses iperf3 --json output into a ThroughputResult (from/to are
// filled by the caller).
func parseIperf3(data []byte) (*ThroughputResult, error) {
	var o iperf3Output
	if err := json.Unmarshal(data, &o); err != nil {
		return nil, fmt.Errorf("failed to parse iperf3 output: %w", err)
	}

	if o.Error != "" {
		return nil, fmt.Errorf("iperf3 error: %s", o.Error)
	}

	sent := o.End.SumSent.BitsPerSecond
	recv := o.End.SumReceived.BitsPerSecond

	res := &ThroughputResult{
		SentBitsPerSec:  sent,
		RecvBitsPerSec:  recv,
		SentMbitsPerSec: sent / 1e6,
		RecvMbitsPerSec: recv / 1e6,
		Retransmits:     o.End.SumSent.Retransmits,
	}

	res.Summary = fmt.Sprintf("%.1f Mbit/s sent, %.1f Mbit/s received, %d retransmits",
		res.SentMbitsPerSec, res.RecvMbitsPerSec, res.Retransmits)

	return res, nil
}

// runThroughput orchestrates an iperf3 test between two deployed nodes using the
// engine's Exec. The target node must have a management IPv4 and both nodes must
// have iperf3 installed (e.g. the network-multitool image).
func runThroughput(ctx context.Context, e Engine, lab, from, to string) (*ThroughputResult, error) {
	status, err := e.Status(ctx, lab)
	if err != nil {
		return nil, err
	}

	var targetIP string

	for _, n := range status.Nodes {
		if n.Name == to {
			targetIP = n.IPv4Address
		}
	}

	if targetIP == "" {
		// Distinguish "runtime down" (503) from "not deployed / no IP" (500).
		if caps := e.Capabilities(ctx); !caps.RuntimeAvailable {
			return nil, fmt.Errorf("%w: %s", ErrRuntimeUnavailable, caps.Reason)
		}

		return nil, fmt.Errorf("target node %q has no management IPv4 (is the lab deployed?)", to)
	}

	// Start a one-shot iperf3 server on the target (daemonizes and returns).
	if _, err := e.Exec(ctx, lab, to, "iperf3 -s -1 -D"); err != nil {
		return nil, fmt.Errorf("failed to start iperf3 server on %q: %w", to, err)
	}

	// Give the server a moment to bind.
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case <-time.After(500 * time.Millisecond):
	}

	// Run the client on the source node.
	clientRes, err := e.Exec(ctx, lab, from, fmt.Sprintf("iperf3 -c %s -J -t 3", targetIP))
	if err != nil {
		return nil, fmt.Errorf("failed to run iperf3 client on %q: %w", from, err)
	}

	res, err := parseIperf3([]byte(clientRes.Stdout))
	if err != nil {
		return nil, err
	}

	res.From = from
	res.To = to
	res.Target = targetIP

	return res, nil
}
