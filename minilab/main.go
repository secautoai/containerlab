// Command minilab is a minimal containerlab: a declarative YAML topology
// becomes docker containers (--network=none) joined by point-to-point veth
// wires, with a deploy / inspect / destroy lifecycle and label-based
// container discovery. Deploy and destroy must run as root (CAP_NET_ADMIN).
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"
	"text/tabwriter"
)

func main() {
	if len(os.Args) < 2 {
		usage()
	}
	cmd := os.Args[1]
	fs := flag.NewFlagSet(cmd, flag.ExitOnError)
	topo := fs.String("t", "", "path to *.clab.yml topology")
	asJSON := fs.Bool("json", false, "JSON output (inspect only)")
	fs.Parse(os.Args[2:])
	if *topo == "" {
		fail(fmt.Errorf("-t <topology file> is required"))
	}
	lab, err := Parse(*topo)
	if err != nil {
		fail(err)
	}
	switch cmd {
	case "deploy":
		err = deploy(lab)
	case "destroy":
		err = destroy(lab)
	case "inspect":
		err = inspect(lab, *asJSON)
	default:
		usage()
	}
	if err != nil {
		fail(err)
	}
}

func usage() {
	fmt.Fprintln(os.Stderr, "usage: minilab deploy|destroy|inspect -t <topology.clab.yml> [--json]")
	os.Exit(2)
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "minilab:", err)
	os.Exit(1)
}

func requireRoot(op string) error {
	if os.Geteuid() != 0 {
		return fmt.Errorf("%s must run as root: veth/netns ops need CAP_NET_ADMIN", op)
	}
	return nil
}

// deploy orders strictly nodes -> links -> execs: every container is created,
// started and confirmed running before any wire is built.
func deploy(lab *Lab) error {
	if err := requireRoot("deploy"); err != nil {
		return err
	}
	nodes := lab.NodeNames()
	for _, n := range nodes {
		if err := createContainer(lab.Name, n, lab.Topology.Nodes[n]); err != nil {
			return err
		}
	}
	for _, n := range nodes {
		if err := startContainer(lab.Name, n); err != nil {
			return err
		}
	}
	pids := make(map[string]int, len(nodes))
	for _, n := range nodes {
		pid, err := waitRunning(containerName(lab.Name, n))
		if err != nil {
			return err
		}
		pids[n] = pid
	}
	for i, ln := range lab.Topology.Links {
		if err := wireLink(lab.Name, i, ln, pids); err != nil {
			return err
		}
	}
	for _, n := range nodes {
		for _, cmd := range lab.Topology.Nodes[n].Exec {
			if err := execDetached(containerName(lab.Name, n), cmd); err != nil {
				return err
			}
		}
	}
	fmt.Printf("lab %q deployed: %d nodes, %d links\n", lab.Name, len(nodes), len(lab.Topology.Links))
	return nil
}

// destroy removes all lab containers (found by label) and sweeps leftover
// root-ns veths carrying the lab prefix. An absent lab still exits 0.
func destroy(lab *Lab) error {
	if err := requireRoot("destroy"); err != nil {
		return err
	}
	names, err := labContainers(lab.Name)
	if err != nil {
		return err
	}
	for _, name := range names {
		if err := removeContainer(name); err != nil {
			return err
		}
	}
	swept, err := sweepVeths(lab.Name)
	if err != nil {
		return err
	}
	fmt.Printf("lab %q destroyed: %d containers removed, %d leftover veths swept\n",
		lab.Name, len(names), swept)
	return nil
}

type nodeStatus struct {
	Node       string     `json:"node"`
	Container  string     `json:"container"`
	State      string     `json:"state"`
	Pid        int        `json:"pid"`
	Interfaces []Endpoint `json:"interfaces"`
}

// inspect reports live container state plus each node's declared wiring.
func inspect(lab *Lab, asJSON bool) error {
	var rows []nodeStatus
	for _, n := range lab.NodeNames() {
		name := containerName(lab.Name, n)
		pid, state, err := pidState(name)
		if err != nil {
			pid, state = 0, "absent"
		}
		rows = append(rows, nodeStatus{n, name, state, pid, lab.IfacesFor(n)})
	}
	if asJSON {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		return enc.Encode(rows)
	}
	tw := tabwriter.NewWriter(os.Stdout, 2, 8, 2, ' ', 0)
	fmt.Fprintln(tw, "NODE\tCONTAINER\tSTATE\tPID\tINTERFACES")
	for _, r := range rows {
		ifs := make([]string, 0, len(r.Interfaces))
		for _, ep := range r.Interfaces {
			s := ep.Iface
			if ep.IPv4 != "" {
				s += ":" + ep.IPv4
			}
			ifs = append(ifs, s)
		}
		if len(ifs) == 0 {
			ifs = []string{"-"}
		}
		fmt.Fprintf(tw, "%s\t%s\t%s\t%d\t%s\n", r.Node, r.Container, r.State, r.Pid, strings.Join(ifs, ","))
	}
	return tw.Flush()
}
