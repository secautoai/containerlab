// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"testing"
)

func TestPingSucceeded(t *testing.T) {
	cases := []struct {
		name   string
		output string
		want   bool
	}{
		{"iproute2 success", "2 packets transmitted, 2 received, 0% packet loss, time 1001ms", true},
		{"busybox success", "2 packets transmitted, 2 packets received, 0% packet loss", true},
		{"total loss", "2 packets transmitted, 0 received, 100% packet loss", false},
		{"unreachable", "connect: Network is unreachable", false},
		{"empty", "", false},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := pingSucceeded(c.output); got != c.want {
				t.Errorf("pingSucceeded(%q) = %v, want %v", c.output, got, c.want)
			}
		})
	}
}

func TestFakeValidate(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	// not deployed yet
	rep, err := e.Validate(ctx, "lab1")
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if rep.Deployed {
		t.Fatal("should not be deployed")
	}

	_, _ = e.Deploy(ctx, "lab1")

	rep, _ = e.Validate(ctx, "lab1")
	// 2 nodes => 2 ordered pairs, all pass
	if rep.Passed != 2 || rep.Failed != 0 {
		t.Fatalf("expected 2 passed, got %+v", rep)
	}

	if rep.Summary != "all reachability checks passed" {
		t.Errorf("unexpected summary: %q", rep.Summary)
	}
}

func TestImpairmentParamsValidate(t *testing.T) {
	cases := []struct {
		name    string
		p       ImpairmentParams
		wantErr bool
	}{
		{"ok", ImpairmentParams{DelayMs: 50, JitterMs: 5, LossPct: 1}, false},
		{"loss too high", ImpairmentParams{LossPct: 150}, true},
		{"corruption too high", ImpairmentParams{CorruptionPct: 101}, true},
		{"jitter without delay", ImpairmentParams{JitterMs: 5}, true},
		{"zero clears", ImpairmentParams{}, false},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			err := c.p.Validate()
			if (err != nil) != c.wantErr {
				t.Errorf("Validate() err=%v, wantErr=%v", err, c.wantErr)
			}
		})
	}
}

func TestFakeSetImpairment(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	// not deployed
	if err := e.SetImpairment(ctx, "lab1", "n1", "eth1", ImpairmentParams{DelayMs: 10}); err == nil {
		t.Fatal("expected error: not deployed")
	}

	_, _ = e.Deploy(ctx, "lab1")

	if err := e.SetImpairment(ctx, "lab1", "n1", "eth1", ImpairmentParams{DelayMs: 10}); err != nil {
		t.Fatalf("set impairment: %v", err)
	}

	// invalid params rejected regardless of deploy state
	if err := e.SetImpairment(ctx, "lab1", "n1", "eth1", ImpairmentParams{LossPct: 200}); err == nil {
		t.Fatal("expected validation error")
	}

	// missing interface rejected
	if err := e.SetImpairment(ctx, "lab1", "n1", "", ImpairmentParams{}); err == nil {
		t.Fatal("expected interface-required error")
	}
}

func TestFakeNodeLifecycle(t *testing.T) {
	ctx := context.Background()
	e := NewFakeEngine(true)
	_ = e.SaveLab(ctx, sampleGraph("lab1"))

	if err := e.NodeLifecycle(ctx, "lab1", "n1", "restart"); err == nil {
		t.Fatal("expected error: lab not deployed")
	}

	_, _ = e.Deploy(ctx, "lab1")

	if err := e.NodeLifecycle(ctx, "lab1", "n1", "restart"); err != nil {
		t.Fatalf("restart: %v", err)
	}

	if err := e.NodeLifecycle(ctx, "lab1", "n1", "bogus"); err == nil {
		t.Fatal("expected error for invalid action")
	}
}
