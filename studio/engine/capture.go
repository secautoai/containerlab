// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import (
	"bytes"
	"context"
	"fmt"

	dockercontainer "github.com/docker/docker/api/types/container"
	dockerclient "github.com/docker/docker/client"
	"github.com/docker/docker/pkg/stdcopy"
)

// captureMaxPackets bounds how many packets a single capture may request.
const captureMaxPackets = 10000

// captureTimeoutSecs bounds how long tcpdump runs so a quiet link can't hang the
// request forever (relies on the `timeout` coreutil being present in the image).
const captureTimeoutSecs = 20

// buildCaptureCmd builds the tcpdump command that writes a pcap stream to stdout.
// It captures `count` packets on `iface` (or until the timeout elapses).
func buildCaptureCmd(iface string, count int) (string, error) {
	if iface == "" {
		return "", fmt.Errorf("interface is required")
	}

	if count <= 0 {
		count = 20
	}

	if count > captureMaxPackets {
		count = captureMaxPackets
	}

	// -w - : pcap to stdout; -U : packet-buffered; -n : no name resolution.
	return fmt.Sprintf("timeout %d tcpdump -i %s -c %d -w - -U -n",
		captureTimeoutSecs, iface, count), nil
}

// pcapMagics are the classic libpcap file magic numbers (micro/nanosecond
// timestamps, both byte orders).
var pcapMagics = [][]byte{
	{0xd4, 0xc3, 0xb2, 0xa1}, // microsec, little-endian
	{0xa1, 0xb2, 0xc3, 0xd4}, // microsec, big-endian
	{0x4d, 0x3c, 0xb2, 0xa1}, // nanosec, little-endian
	{0xa1, 0xb2, 0x3c, 0x4d}, // nanosec, big-endian
}

// isPcap reports whether data begins with a libpcap file header magic number.
func isPcap(data []byte) bool {
	if len(data) < 4 {
		return false
	}

	for _, m := range pcapMagics {
		if bytes.Equal(data[:4], m) {
			return true
		}
	}

	return false
}

// minimalPcap returns a valid, empty classic pcap file (global header only).
func minimalPcap() []byte {
	return []byte{
		0xd4, 0xc3, 0xb2, 0xa1, // magic (microsec, LE)
		0x02, 0x00, 0x04, 0x00, // version 2.4
		0x00, 0x00, 0x00, 0x00, // thiszone
		0x00, 0x00, 0x00, 0x00, // sigfigs
		0xff, 0xff, 0x00, 0x00, // snaplen 65535
		0x01, 0x00, 0x00, 0x00, // network (Ethernet)
	}
}

// captureViaDocker runs the capture command in the target container and returns
// the raw pcap bytes from stdout.
func captureViaDocker(ctx context.Context, container, cmd string) ([]byte, error) {
	cli, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrRuntimeUnavailable, err)
	}
	defer cli.Close()

	if _, perr := cli.Ping(ctx); perr != nil {
		return nil, fmt.Errorf("%w: %v", ErrRuntimeUnavailable, perr)
	}

	execID, err := cli.ContainerExecCreate(ctx, container, dockercontainer.ExecOptions{
		User:         "0",
		AttachStdout: true,
		AttachStderr: true,
		Cmd:          []string{"sh", "-c", cmd},
	})
	if err != nil {
		return nil, err
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

	data := outBuf.Bytes()
	if !isPcap(data) {
		msg := errBuf.String()
		if msg == "" {
			msg = "no pcap data captured (is tcpdump installed in the node image?)"
		}

		return nil, fmt.Errorf("capture failed: %s", msg)
	}

	return data, nil
}
