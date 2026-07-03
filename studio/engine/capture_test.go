// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"strings"
	"testing"
)

func TestBuildCaptureCmd(t *testing.T) {
	cmd, err := buildCaptureCmd("eth1", 20)
	if err != nil {
		t.Fatalf("build: %v", err)
	}

	for _, want := range []string{"tcpdump", "-i eth1", "-c 20", "-w -", "timeout"} {
		if !strings.Contains(cmd, want) {
			t.Errorf("cmd %q missing %q", cmd, want)
		}
	}

	// empty interface rejected
	if _, err := buildCaptureCmd("", 10); err == nil {
		t.Error("expected error for empty interface")
	}

	// count is clamped
	cmd, _ = buildCaptureCmd("eth1", 999999)
	if !strings.Contains(cmd, "-c 10000") {
		t.Errorf("expected count clamp to 10000, got %q", cmd)
	}

	// non-positive count defaults to 20
	cmd, _ = buildCaptureCmd("eth1", 0)
	if !strings.Contains(cmd, "-c 20") {
		t.Errorf("expected default count 20, got %q", cmd)
	}
}

func TestIsPcap(t *testing.T) {
	if !isPcap(minimalPcap()) {
		t.Error("minimalPcap should be recognized as pcap")
	}

	// big-endian nanosecond magic
	if !isPcap([]byte{0xa1, 0xb2, 0x3c, 0x4d, 0, 0}) {
		t.Error("nsec BE magic should be recognized")
	}

	if isPcap([]byte("not a pcap")) {
		t.Error("text should not be recognized as pcap")
	}

	if isPcap([]byte{0x01, 0x02}) {
		t.Error("short input should not be pcap")
	}
}

func TestFakeCapture(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	// invalid interface rejected regardless of state
	if _, err := e.Capture(ctx, "lab1", "n1", "", 10); err == nil {
		t.Fatal("expected error for empty interface")
	}

	// not deployed
	if _, err := e.Capture(ctx, "lab1", "n1", "eth1", 10); err == nil {
		t.Fatal("expected error before deploy")
	}

	_, _ = e.Deploy(ctx, "lab1")

	data, err := e.Capture(ctx, "lab1", "n1", "eth1", 10)
	if err != nil {
		t.Fatalf("capture: %v", err)
	}

	if !isPcap(data) {
		t.Fatalf("expected valid pcap bytes, got %d bytes", len(data))
	}
}

func TestFakeCaptureRuntimeDown(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(false)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	if _, err := e.Capture(ctx, "lab1", "n1", "eth1", 10); err == nil {
		t.Fatal("expected ErrRuntimeUnavailable")
	}
}
