package main

import (
	"fmt"
	"strings"

	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

const linkMTU = 9500 // jumbo, like containerlab's default

// vethPrefix is the deterministic root-ns temp-name prefix of a lab's wires.
// Names must fit IFNAMSIZ-1 = 15 chars: "ml-" + lab[:8] + "-" + idx + "a"|"b".
func vethPrefix(lab string) string {
	return "ml-" + lab[:min(8, len(lab))] + "-"
}

// wireLink builds one veth pair in the root ns, then pushes and configures
// each end inside its endpoint's container netns. Stale temp names from a
// crashed earlier run are deleted first.
func wireLink(lab string, idx int, ln *Link, pids map[string]int) error {
	nameA := fmt.Sprintf("%s%da", vethPrefix(lab), idx)
	nameB := fmt.Sprintf("%s%db", vethPrefix(lab), idx)
	for _, n := range []string{nameA, nameB} {
		if old, err := netlink.LinkByName(n); err == nil {
			if err := netlink.LinkDel(old); err != nil {
				return fmt.Errorf("delete stale %s: %w", n, err)
			}
		}
	}
	veth := &netlink.Veth{
		LinkAttrs: netlink.LinkAttrs{Name: nameA, MTU: linkMTU},
		PeerName:  nameB,
	}
	if err := netlink.LinkAdd(veth); err != nil {
		return fmt.Errorf("create veth %s<->%s: %w", nameA, nameB, err)
	}
	for i, tmp := range []string{nameA, nameB} {
		ep := ln.EP(i)
		if err := placeEnd(tmp, ep, pids[ep.Node]); err != nil {
			return fmt.Errorf("link %d (%s:%s): %w", idx, ep.Node, ep.Iface, err)
		}
	}
	return nil
}

// placeEnd moves the veth end named tmp into pid's netns, renames it to the
// declared iface, optionally adds the IPv4, and brings it up. In-namespace
// ops go through a namespaced netlink handle — no thread ns switching.
func placeEnd(tmp string, ep Endpoint, pid int) error {
	link, err := netlink.LinkByName(tmp)
	if err != nil {
		return fmt.Errorf("find %s in root ns: %w", tmp, err)
	}
	nsh, err := netns.GetFromPid(pid)
	if err != nil {
		return fmt.Errorf("open netns of pid %d: %w", pid, err)
	}
	defer nsh.Close()
	if err := netlink.LinkSetNsFd(link, int(nsh)); err != nil {
		return fmt.Errorf("move %s into netns: %w", tmp, err)
	}
	h, err := netlink.NewHandleAt(nsh)
	if err != nil {
		return fmt.Errorf("netlink handle in netns: %w", err)
	}
	defer h.Close()
	inside, err := h.LinkByName(tmp)
	if err != nil {
		return fmt.Errorf("find %s inside netns: %w", tmp, err)
	}
	if err := h.LinkSetName(inside, ep.Iface); err != nil {
		return fmt.Errorf("rename to %s: %w", ep.Iface, err)
	}
	if ep.IPv4 != "" {
		addr, err := netlink.ParseAddr(ep.IPv4)
		if err != nil {
			return err
		}
		if err := h.AddrAdd(inside, addr); err != nil {
			return fmt.Errorf("add %s: %w", ep.IPv4, err)
		}
	}
	if err := h.LinkSetUp(inside); err != nil {
		return fmt.Errorf("set %s up: %w", ep.Iface, err)
	}
	return nil
}

// sweepVeths deletes leftover root-ns links carrying the lab's veth prefix.
// Healthy ends live inside container netns's, so any match here is debris
// from a crashed deploy. Returns how many links were deleted.
func sweepVeths(lab string) (int, error) {
	links, err := netlink.LinkList()
	if err != nil {
		return 0, fmt.Errorf("list root-ns links: %w", err)
	}
	n := 0
	for _, l := range links {
		name := l.Attrs().Name
		if !strings.HasPrefix(name, vethPrefix(lab)) {
			continue
		}
		// Re-lookup: deleting one end of a pair whose peer is also in the
		// root ns removes both, so a listed link may already be gone.
		cur, err := netlink.LinkByName(name)
		if err != nil {
			continue
		}
		if err := netlink.LinkDel(cur); err != nil {
			return n, fmt.Errorf("delete leftover %s: %w", name, err)
		}
		n++
	}
	return n, nil
}
