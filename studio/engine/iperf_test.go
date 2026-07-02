// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"testing"
)

// a trimmed but structurally-real iperf3 --json output
const iperf3Sample = `{
  "start": {"connected": [{"socket": 5}]},
  "end": {
    "sum_sent":     {"start": 0, "end": 3.0, "bytes": 3600000000, "bits_per_second": 9600000000, "retransmits": 12},
    "sum_received": {"start": 0, "end": 3.0, "bytes": 3550000000, "bits_per_second": 9466666666}
  }
}`

func TestParseIperf3(t *testing.T) {
	res, err := parseIperf3([]byte(iperf3Sample))
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if res.Retransmits != 12 {
		t.Errorf("expected 12 retransmits, got %d", res.Retransmits)
	}

	if res.SentMbitsPerSec < 9599 || res.SentMbitsPerSec > 9601 {
		t.Errorf("unexpected sent Mbit/s: %f", res.SentMbitsPerSec)
	}

	if res.RecvMbitsPerSec < 9466 || res.RecvMbitsPerSec > 9467 {
		t.Errorf("unexpected recv Mbit/s: %f", res.RecvMbitsPerSec)
	}

	if res.Summary == "" {
		t.Error("expected a summary")
	}
}

func TestParseIperf3Error(t *testing.T) {
	if _, err := parseIperf3([]byte(`{"error":"unable to connect to server"}`)); err == nil {
		t.Fatal("expected error from iperf3 error field")
	}

	if _, err := parseIperf3([]byte("not json")); err == nil {
		t.Fatal("expected parse error for invalid json")
	}
}

func TestFakeThroughput(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	if _, err := e.Throughput(ctx, "lab1", "n1", "n2"); err == nil {
		t.Fatal("expected error before deploy")
	}

	_, _ = e.Deploy(ctx, "lab1")

	res, err := e.Throughput(ctx, "lab1", "n1", "n2")
	if err != nil {
		t.Fatalf("throughput: %v", err)
	}

	if res.SentMbitsPerSec <= 0 {
		t.Errorf("expected positive throughput, got %f", res.SentMbitsPerSec)
	}
}
