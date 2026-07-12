package main

import (
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"time"
)

// Discovery labels: every lookup goes by label, never by parsing names.
const (
	labelLab  = "minilab.lab"
	labelNode = "minilab.node"
)

func containerName(lab, node string) string { return "minilab-" + lab + "-" + node }

// docker runs the docker CLI and returns its trimmed combined output.
func docker(args ...string) (string, error) {
	out, err := exec.Command("docker", args...).CombinedOutput()
	s := strings.TrimSpace(string(out))
	if err != nil {
		return s, fmt.Errorf("docker %s: %w (%s)", args[0], err, s)
	}
	return s, nil
}

// createContainer creates (without starting) one node's container, detached
// from every docker network: minilab's veth wires are the only dataplane.
func createContainer(lab, node string, n *Node) error {
	_, err := docker("create", "--network=none",
		"--name", containerName(lab, node),
		"--label", labelLab+"="+lab,
		"--label", labelNode+"="+node,
		"--hostname", node,
		n.Image)
	return err
}

func startContainer(lab, node string) error {
	_, err := docker("start", containerName(lab, node))
	return err
}

// labContainers returns the names of a lab's containers in any state.
func labContainers(lab string) ([]string, error) {
	out, err := docker("ps", "-a", "--filter", "label="+labelLab+"="+lab,
		"--format", "{{.Names}}")
	if err != nil || out == "" {
		return nil, err
	}
	return strings.Split(out, "\n"), nil
}

// pidState returns a container's init PID and status ("running", ...).
func pidState(name string) (int, string, error) {
	out, err := docker("inspect", "-f", "{{.State.Pid}} {{.State.Status}}", name)
	if err != nil {
		return 0, "", err
	}
	pidS, state, _ := strings.Cut(out, " ")
	pid, err := strconv.Atoi(pidS)
	if err != nil {
		return 0, "", fmt.Errorf("inspect %s: bad pid %q", name, pidS)
	}
	return pid, state, nil
}

// waitRunning polls until the container runs with a live PID whose netns is
// visible in /proc — a just-started container can briefly report PID 0 or an
// unpopulated /proc/<pid>/ns/net.
func waitRunning(name string) (int, error) {
	var last string
	for range 20 { // bounded: 20 x 100ms
		pid, state, err := pidState(name)
		switch {
		case err != nil:
			last = err.Error()
		case state != "running" || pid <= 0:
			last = fmt.Sprintf("state=%s pid=%d", state, pid)
		default:
			if _, err := os.Stat(fmt.Sprintf("/proc/%d/ns/net", pid)); err == nil {
				return pid, nil
			}
			last = fmt.Sprintf("/proc/%d/ns/net not there yet", pid)
		}
		time.Sleep(100 * time.Millisecond)
	}
	return 0, fmt.Errorf("%s: not running after 2s (%s)", name, last)
}

// execDetached runs one exec entry via `docker exec -d`. Naive whitespace
// argv split — the nodeagent image has no shell to do it for us.
func execDetached(name, command string) error {
	argv := strings.Fields(command)
	if len(argv) == 0 {
		return nil
	}
	_, err := docker(append([]string{"exec", "-d", name}, argv...)...)
	return err
}

// removeContainer force-removes one container; already absent counts as
// success so destroy stays idempotent.
func removeContainer(name string) error {
	out, err := docker("rm", "-f", name)
	if err != nil && strings.Contains(out, "No such container") {
		return nil
	}
	return err
}
