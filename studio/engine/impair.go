// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"context"
	"fmt"
	"net"
	"os/exec"
	"time"

	"github.com/containernetworking/plugins/pkg/ns"
	clabcore "github.com/srl-labs/containerlab/core"
	clablinks "github.com/srl-labs/containerlab/links"
	clabnetem "github.com/srl-labs/containerlab/netem"
	clabruntime "github.com/srl-labs/containerlab/runtime"
	"github.com/vishvananda/netlink"
)

// SetImpairment applies netem link impairments to a node's interface by
// entering the container's network namespace and installing a tc netem qdisc.
//
// It mirrors the logic of `containerlab tools netem set` but is driven by the
// ClabStudio engine. Passing an all-zero params effectively clears impairments
// (netem is replaced with zeroed values). Requires root privileges and a
// running container.
func (e *ClabEngine) SetImpairment(
	ctx context.Context,
	lab, node, iface string,
	params ImpairmentParams,
) error {
	if err := params.Validate(); err != nil {
		return err
	}

	if iface == "" {
		return fmt.Errorf("interface is required")
	}

	target, err := e.ConsoleTarget(ctx, lab, node)
	if err != nil {
		return err
	}

	// Best-effort: ensure the sch_netem kernel module is available.
	if merr := exec.CommandContext(ctx, "modprobe", "sch_netem").Run(); merr != nil {
		// non-fatal; the module may be built-in or already loaded
		_ = merr
	}

	_, rinit, err := clabcore.RuntimeInitializer(e.runtime)
	if err != nil {
		return err
	}

	rt := rinit()
	if err := rt.Init(clabruntime.WithConfig(&clabruntime.RuntimeConfig{Timeout: e.timeout})); err != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	nsPath, err := rt.GetNSPath(ctx, target.Container)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	nodeNs, err := ns.GetNS(nsPath)
	if err != nil {
		return err
	}
	defer nodeNs.Close()

	tcnl, err := clabnetem.NewTC(int(nodeNs.Fd()))
	if err != nil {
		return err
	}

	defer tcnl.Close()

	delay := time.Duration(params.DelayMs) * time.Millisecond
	jitter := time.Duration(params.JitterMs) * time.Millisecond

	return nodeNs.Do(func(_ ns.NetNS) error {
		link, err := netlink.LinkByName(clablinks.SanitizeInterfaceName(iface))
		if err != nil {
			return fmt.Errorf("interface %q not found on node %q: %w", iface, node, err)
		}

		netIf, err := net.InterfaceByName(link.Attrs().Name)
		if err != nil {
			return err
		}

		_, err = clabnetem.SetImpairments(
			tcnl,
			target.Container,
			netIf,
			delay,
			jitter,
			params.LossPct,
			params.RateKbit,
			params.CorruptionPct,
		)

		return err
	})
}
