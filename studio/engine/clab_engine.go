// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	dockercontainer "github.com/docker/docker/api/types/container"
	dockerclient "github.com/docker/docker/client"
	"github.com/docker/docker/pkg/stdcopy"
	"github.com/google/shlex"
	clabconstants "github.com/srl-labs/containerlab/constants"
	clabcore "github.com/srl-labs/containerlab/core"
	clabruntime "github.com/srl-labs/containerlab/runtime"
	"github.com/srl-labs/containerlab/studio/model"
)

// ClabEngine is the production Engine backed by containerlab's core.CLab. Labs
// are stored on disk under <labsDir>/<name>/<name>.clab.yml, following common
// containerlab conventions.
type ClabEngine struct {
	labsDir string
	runtime string
	timeout time.Duration
	owner   string
}

// Config configures a ClabEngine.
type Config struct {
	LabsDir string
	Runtime string
	Timeout time.Duration
	Owner   string
}

// NewClabEngine creates a filesystem-backed containerlab engine. The labs
// directory is created if it does not exist.
func NewClabEngine(cfg Config) (*ClabEngine, error) {
	if cfg.LabsDir == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, fmt.Errorf("could not determine home dir: %w", err)
		}

		cfg.LabsDir = filepath.Join(home, ".clab", "studio")
	}

	if cfg.Timeout == 0 {
		cfg.Timeout = 120 * time.Second
	}

	abs, err := filepath.Abs(cfg.LabsDir)
	if err != nil {
		return nil, err
	}

	if err := os.MkdirAll(abs, 0o755); err != nil {
		return nil, fmt.Errorf("could not create labs dir %q: %w", abs, err)
	}

	return &ClabEngine{
		labsDir: abs,
		runtime: cfg.Runtime,
		timeout: cfg.Timeout,
		owner:   cfg.Owner,
	}, nil
}

var _ Engine = (*ClabEngine)(nil)

// LabsDir returns the directory where labs are stored.
func (e *ClabEngine) LabsDir() string { return e.labsDir }

// topoPath returns the on-disk path of a lab's topology file.
func (e *ClabEngine) topoPath(name string) string {
	return filepath.Join(e.labsDir, name, name+".clab.yml")
}

// Capabilities probes the container runtime for availability.
func (e *ClabEngine) Capabilities(ctx context.Context) Capabilities {
	name, _, _ := clabcore.RuntimeInitializer(e.runtime)
	c := Capabilities{Runtime: name}

	clab, err := e.baseCLab(name)
	if err != nil {
		c.Reason = err.Error()
		return c
	}

	cctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	if err := clab.CheckConnectivity(cctx); err != nil {
		c.Reason = err.Error()
		return c
	}

	c.RuntimeAvailable = true

	return c
}

// baseCLab builds a minimal CLab with just a runtime configured (no topology).
func (e *ClabEngine) baseCLab(_ string) (*clabcore.CLab, error) {
	return clabcore.NewContainerLab(
		clabcore.WithTimeout(e.timeout),
		clabcore.WithRuntime(e.runtime, &clabruntime.RuntimeConfig{Timeout: e.timeout}),
	)
}

// labCLabFromFile builds a CLab from a lab's on-disk topology file.
func (e *ClabEngine) labCLabFromFile(name string) (*clabcore.CLab, error) {
	path := e.topoPath(name)
	if _, err := os.Stat(path); err != nil {
		return nil, fmt.Errorf("%w: lab %q", ErrNotFound, name)
	}

	opts := []clabcore.ClabOption{
		clabcore.WithTimeout(e.timeout),
		clabcore.WithRuntime(e.runtime, &clabruntime.RuntimeConfig{Timeout: e.timeout}),
		clabcore.WithTopoPath(path, nil),
	}

	if e.owner != "" {
		opts = append(opts, clabcore.WithLabOwner(e.owner))
	}

	return clabcore.NewContainerLab(opts...)
}

// ListLabs scans the labs directory and marks labs that are currently deployed.
func (e *ClabEngine) ListLabs(ctx context.Context) ([]LabSummary, error) {
	entries, err := os.ReadDir(e.labsDir)
	if err != nil {
		return nil, err
	}

	deployed := e.deployedLabNames(ctx)

	out := make([]LabSummary, 0, len(entries))

	for _, ent := range entries {
		if !ent.IsDir() {
			continue
		}

		name := ent.Name()

		path := e.topoPath(name)
		data, err := os.ReadFile(path)
		if err != nil {
			continue // not a studio lab dir
		}

		g, err := model.ClabYAMLToGraph(data)
		if err != nil {
			continue
		}

		_, isDeployed := deployed[name]

		out = append(out, LabSummary{
			Name:      name,
			Path:      path,
			NodeCount: len(g.Nodes),
			Deployed:  isDeployed,
			Owner:     e.owner,
			State:     stateFor(isDeployed),
		})
	}

	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })

	return out, nil
}

// deployedLabNames returns the set of lab names that have running containers.
// On any runtime error it returns an empty set (labs still show as defined).
func (e *ClabEngine) deployedLabNames(ctx context.Context) map[string]struct{} {
	result := map[string]struct{}{}

	clab, err := e.baseCLab(e.runtime)
	if err != nil {
		return result
	}

	containers, err := clab.ListContainers(ctx, clabcore.WithListclabLabelExists())
	if err != nil {
		return result
	}

	for i := range containers {
		if lab := containers[i].Labels[clabconstants.Containerlab]; lab != "" {
			result[lab] = struct{}{}
		}
	}

	return result
}

// GetLab reads and parses a lab's topology into the UI graph model.
func (e *ClabEngine) GetLab(_ context.Context, name string) (*model.Graph, error) {
	data, err := os.ReadFile(e.topoPath(name))
	if err != nil {
		return nil, fmt.Errorf("%w: lab %q", ErrNotFound, name)
	}

	return model.ClabYAMLToGraph(data)
}

// SaveLab writes a lab graph to disk as a containerlab topology file.
func (e *ClabEngine) SaveLab(_ context.Context, g *model.Graph) error {
	if g == nil || strings.TrimSpace(g.Name) == "" {
		return fmt.Errorf("lab name is required")
	}

	if err := validateLabName(g.Name); err != nil {
		return err
	}

	data, err := model.GraphToClabYAML(g)
	if err != nil {
		return err
	}

	dir := filepath.Join(e.labsDir, g.Name)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}

	return os.WriteFile(e.topoPath(g.Name), data, 0o644)
}

// DeleteLab removes a lab directory from disk. The lab must not be deployed.
func (e *ClabEngine) DeleteLab(ctx context.Context, name string) error {
	if err := validateLabName(name); err != nil {
		return err
	}

	if _, deployed := e.deployedLabNames(ctx)[name]; deployed {
		return fmt.Errorf("lab %q is deployed; destroy it first", name)
	}

	dir := filepath.Join(e.labsDir, name)
	if _, err := os.Stat(dir); err != nil {
		return fmt.Errorf("%w: lab %q", ErrNotFound, name)
	}

	return os.RemoveAll(dir)
}

// RenderYAML returns the raw topology YAML for a lab.
func (e *ClabEngine) RenderYAML(_ context.Context, name string) ([]byte, error) {
	data, err := os.ReadFile(e.topoPath(name))
	if err != nil {
		return nil, fmt.Errorf("%w: lab %q", ErrNotFound, name)
	}

	return data, nil
}

// Deploy deploys a lab from its on-disk topology.
func (e *ClabEngine) Deploy(ctx context.Context, name string) (*LabStatus, error) {
	clab, err := e.labCLabFromFile(name)
	if err != nil {
		return nil, err
	}

	if err := clab.CheckConnectivity(ctx); err != nil {
		return nil, fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	deployOpts, err := clabcore.NewDeployOptions(0)
	if err != nil {
		return nil, err
	}

	if _, err := clab.Deploy(ctx, deployOpts); err != nil {
		return nil, err
	}

	return e.Status(ctx, name)
}

// Destroy tears down a deployed lab.
func (e *ClabEngine) Destroy(ctx context.Context, name string, cleanup bool) error {
	clab, err := e.labCLabFromFile(name)
	if err != nil {
		return err
	}

	if err := clab.CheckConnectivity(ctx); err != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	opts := []clabcore.DestroyOption{clabcore.WithDestroyMaxWorkers(0)}
	if cleanup {
		opts = append(opts, clabcore.WithDestroyCleanup())
	}

	return clab.Destroy(ctx, opts...)
}

// Status returns the runtime status of a lab by inspecting its containers.
func (e *ClabEngine) Status(ctx context.Context, name string) (*LabStatus, error) {
	g, err := e.GetLab(ctx, name)
	if err != nil {
		return nil, err
	}

	st := &LabStatus{Name: name}

	// index of defined nodes for merging runtime info
	nodeByName := map[string]*NodeStatus{}
	for _, n := range g.Nodes {
		ns := NodeStatus{Name: n.Name, Kind: n.Kind, Image: n.Image, State: "stopped"}
		st.Nodes = append(st.Nodes, ns)
	}

	for i := range st.Nodes {
		nodeByName[st.Nodes[i].Name] = &st.Nodes[i]
	}

	clab, err := e.baseCLab(e.runtime)
	if err != nil {
		return st, nil // design-only info
	}

	containers, err := clab.ListContainers(ctx, clabcore.WithListLabName(name))
	if err != nil {
		return st, nil // runtime down; return defined-only status
	}

	for i := range containers {
		nn := containers[i].Labels[clabconstants.NodeName]

		ns, ok := nodeByName[nn]
		if !ok {
			continue
		}

		st.Deployed = true
		ns.State = fmt.Sprintf("%s/%s", containers[i].State, containers[i].Status)
		ns.IPv4Address = containers[i].GetContainerIPv4()
		ns.IPv6Address = containers[i].GetContainerIPv6()
	}

	return st, nil
}

// Exec runs a command on a single node of a deployed lab and returns its
// stdout, stderr and exit code.
func (e *ClabEngine) Exec(ctx context.Context, lab, node, cmd string) (*ExecResult, error) {
	target, err := e.ConsoleTarget(ctx, lab, node)
	if err != nil {
		return nil, err
	}

	parts, err := shlex.Split(cmd)
	if err != nil || len(parts) == 0 {
		return nil, fmt.Errorf("invalid command %q", cmd)
	}

	cli, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}
	defer cli.Close()

	execID, err := cli.ContainerExecCreate(ctx, target.Container, dockercontainer.ExecOptions{
		User:         "0",
		AttachStdout: true,
		AttachStderr: true,
		Cmd:          parts,
	})
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	resp, err := cli.ContainerExecAttach(ctx, execID.ID, dockercontainer.ExecStartOptions{})
	if err != nil {
		return nil, err
	}
	defer resp.Close()

	var outBuf, errBuf bytes.Buffer
	if _, err := stdcopy.StdCopy(&outBuf, &errBuf, resp.Reader); err != nil {
		return nil, err
	}

	res := &ExecResult{
		Node:   node,
		Cmd:    cmd,
		Stdout: outBuf.String(),
		Stderr: errBuf.String(),
	}

	if inspect, ierr := cli.ContainerExecInspect(ctx, execID.ID); ierr == nil {
		res.ReturnCode = inspect.ExitCode
	}

	return res, nil
}

// ConsoleTarget resolves the real container name for a node and a default
// interactive command derived from the node's kind.
func (e *ClabEngine) ConsoleTarget(ctx context.Context, lab, node string) (*ConsoleTarget, error) {
	g, err := e.GetLab(ctx, lab)
	if err != nil {
		return nil, err
	}

	var kind string

	for _, n := range g.Nodes {
		if n.Name == node {
			kind = n.Kind
			break
		}
	}

	if kind == "" {
		return nil, fmt.Errorf("%w: node %q in lab %q", ErrNotFound, node, lab)
	}

	target := &ConsoleTarget{
		// containerlab's default container naming convention.
		Container: fmt.Sprintf("clab-%s-%s", lab, node),
		Cmd:       ConsoleCommandForKind(kind),
	}

	// Prefer the authoritative container name from the runtime when reachable.
	clab, cerr := e.baseCLab(e.runtime)
	if cerr == nil {
		containers, lerr := clab.ListContainers(ctx, clabcore.WithListLabName(lab))
		if lerr == nil {
			for i := range containers {
				if containers[i].Labels[clabconstants.NodeName] == node &&
					len(containers[i].Names) > 0 {
					target.Container = containers[i].Names[0]
					break
				}
			}
		}
	}

	return target, nil
}

// Validate runs a node-to-node ping matrix across a deployed lab and reports
// reachability using each node's management IPv4 address as the target.
func (e *ClabEngine) Validate(ctx context.Context, lab string) (*ValidationReport, error) {
	status, err := e.Status(ctx, lab)
	if err != nil {
		return nil, err
	}

	report := &ValidationReport{Lab: lab, Deployed: status.Deployed}
	if !status.Deployed {
		report.summarize()
		return report, nil
	}

	// Build a target IP map for reachable nodes.
	type target struct{ node, ip string }

	targets := make([]target, 0, len(status.Nodes))

	for _, n := range status.Nodes {
		if n.IPv4Address != "" {
			targets = append(targets, target{node: n.Name, ip: n.IPv4Address})
		}
	}

	for _, from := range status.Nodes {
		if from.IPv4Address == "" {
			continue // node has no address / not running
		}

		for _, to := range targets {
			if to.node == from.Name {
				continue
			}

			res, execErr := e.Exec(ctx, lab, from.Name,
				fmt.Sprintf("ping -c 2 -W 1 %s", to.ip))

			check := ReachabilityCheck{From: from.Name, To: to.node, Target: to.ip}

			switch {
			case execErr != nil:
				check.OK = false
				check.Detail = execErr.Error()
			default:
				check.OK = pingSucceeded(res.Stdout + "\n" + res.Stderr)
			}

			if check.OK {
				report.Passed++
			} else {
				report.Failed++
			}

			report.Checks = append(report.Checks, check)
		}
	}

	report.summarize()

	return report, nil
}

// NodeLifecycle starts, stops or restarts a single node's container.
func (e *ClabEngine) NodeLifecycle(ctx context.Context, lab, node, action string) error {
	if !isValidAction(action) {
		return fmt.Errorf("invalid action %q (want start|stop|restart)", action)
	}

	target, err := e.ConsoleTarget(ctx, lab, node)
	if err != nil {
		return err
	}

	cli, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}
	defer cli.Close()

	// Detect an unreachable runtime up-front so the API returns 503 (not 500).
	if _, perr := cli.Ping(ctx); perr != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, perr)
	}

	switch action {
	case "start":
		return cli.ContainerStart(ctx, target.Container, dockercontainer.StartOptions{})
	case "stop":
		return cli.ContainerStop(ctx, target.Container, dockercontainer.StopOptions{})
	case "restart":
		return cli.ContainerRestart(ctx, target.Container, dockercontainer.StopOptions{})
	}

	return nil
}

// SaveConfigs persists the running configuration of a deployed lab's nodes.
func (e *ClabEngine) SaveConfigs(ctx context.Context, lab string) error {
	clab, err := e.labCLabFromFile(lab)
	if err != nil {
		return err
	}

	if err := clab.CheckConnectivity(ctx); err != nil {
		return fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}

	return clab.Save(ctx)
}

// CloneLab copies a lab's topology into a new lab directory.
func (e *ClabEngine) CloneLab(ctx context.Context, src, dst string) error {
	if err := validateLabName(dst); err != nil {
		return err
	}

	if _, err := os.Stat(filepath.Join(e.labsDir, dst)); err == nil {
		return fmt.Errorf("lab %q already exists", dst)
	}

	g, err := e.GetLab(ctx, src)
	if err != nil {
		return err
	}

	g.Name = dst

	return e.SaveLab(ctx, g)
}

// RenameLab renames a lab. The lab must not be deployed and the target name must
// not already exist.
func (e *ClabEngine) RenameLab(ctx context.Context, oldName, newName string) error {
	if err := validateLabName(newName); err != nil {
		return err
	}

	if _, deployed := e.deployedLabNames(ctx)[oldName]; deployed {
		return fmt.Errorf("lab %q is deployed; destroy it before renaming", oldName)
	}

	if _, err := os.Stat(filepath.Join(e.labsDir, newName)); err == nil {
		return fmt.Errorf("lab %q already exists", newName)
	}

	if err := e.CloneLab(ctx, oldName, newName); err != nil {
		return err
	}

	return os.RemoveAll(filepath.Join(e.labsDir, oldName))
}

// Throughput runs an iperf3 test between two nodes of a deployed lab.
func (e *ClabEngine) Throughput(ctx context.Context, lab, from, to string) (*ThroughputResult, error) {
	return runThroughput(ctx, e, lab, from, to)
}

// Capture captures packets on a node interface and returns raw pcap bytes.
func (e *ClabEngine) Capture(ctx context.Context, lab, node, iface string, count int) ([]byte, error) {
	cmd, err := buildCaptureCmd(iface, count)
	if err != nil {
		return nil, err
	}

	target, err := e.ConsoleTarget(ctx, lab, node)
	if err != nil {
		return nil, err
	}

	return captureViaDocker(ctx, target.Container, cmd)
}

// validateLabName ensures a lab name is safe for filesystem paths.
func validateLabName(name string) error {
	if name == "" {
		return fmt.Errorf("lab name is required")
	}

	if strings.ContainsAny(name, "/\\.. ") || strings.Contains(name, "..") {
		return fmt.Errorf("invalid lab name %q", name)
	}

	return nil
}
