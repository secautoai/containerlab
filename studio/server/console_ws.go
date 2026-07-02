// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package server

import (
	"context"
	"encoding/json"
	"net/http"
	"sync"
	"time"

	"github.com/charmbracelet/log"
	dockercontainer "github.com/docker/docker/api/types/container"
	dockerclient "github.com/docker/docker/client"
	"github.com/gorilla/websocket"
	clabstudioengine "github.com/srl-labs/containerlab/studio/engine"
)

// consoleUpgrader upgrades console requests to WebSocket. The app is served from
// the same origin, so any origin from the embedded UI is acceptable.
var consoleUpgrader = websocket.Upgrader{
	CheckOrigin:     func(_ *http.Request) bool { return true },
	ReadBufferSize:  4096,
	WriteBufferSize: 4096,
}

// consoleControl is a control message sent by the client (e.g. terminal resize).
type consoleControl struct {
	Type string `json:"type"`
	Cols uint   `json:"cols"`
	Rows uint   `json:"rows"`
}

// registerConsole registers the interactive browser-console WebSocket endpoint.
func registerConsole(mux *http.ServeMux, eng clabstudioengine.Engine) {
	mux.HandleFunc("GET /api/labs/{name}/nodes/{node}/console",
		func(w http.ResponseWriter, r *http.Request) {
			consoleHandler(w, r, eng)
		})
}

func consoleHandler(w http.ResponseWriter, r *http.Request, eng clabstudioengine.Engine) {
	lab := r.PathValue("name")
	node := r.PathValue("node")

	target, err := eng.ConsoleTarget(r.Context(), lab, node)
	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	conn, err := consoleUpgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Debug("console ws upgrade failed", "err", err)
		return
	}
	defer conn.Close()

	if err := attachConsole(r.Context(), conn, target); err != nil {
		writeConsoleError(conn, err.Error())
	}
}

// attachConsole wires a docker exec TTY session to the WebSocket connection.
func attachConsole(
	ctx context.Context,
	conn *websocket.Conn,
	target *clabstudioengine.ConsoleTarget,
) error {
	cli, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return err
	}
	defer cli.Close()

	execID, err := cli.ContainerExecCreate(ctx, target.Container, dockercontainer.ExecOptions{
		User:         "0",
		AttachStdin:  true,
		AttachStdout: true,
		AttachStderr: true,
		Tty:          true,
		Cmd:          target.Cmd,
		Env:          []string{"TERM=xterm-256color"},
	})
	if err != nil {
		return err
	}

	resp, err := cli.ContainerExecAttach(ctx, execID.ID, dockercontainer.ExecStartOptions{Tty: true})
	if err != nil {
		return err
	}
	defer resp.Close()

	writeConsoleInfo(conn, "connected to "+target.Container)

	// A single writer goroutine copies container output to the WebSocket.
	var writeMu sync.Mutex

	done := make(chan struct{})

	go func() {
		defer close(done)

		buf := make([]byte, 4096)
		for {
			n, rerr := resp.Reader.Read(buf)
			if n > 0 {
				writeMu.Lock()
				werr := conn.WriteMessage(websocket.BinaryMessage, buf[:n])
				writeMu.Unlock()
				if werr != nil {
					return
				}
			}
			if rerr != nil {
				return
			}
		}
	}()

	// Read loop: client -> container stdin, plus resize control messages.
	for {
		mt, data, rerr := conn.ReadMessage()
		if rerr != nil {
			break
		}

		if mt == websocket.TextMessage && len(data) > 0 && data[0] == '{' {
			var ctrl consoleControl
			if json.Unmarshal(data, &ctrl) == nil && ctrl.Type == "resize" {
				_ = cli.ContainerExecResize(ctx, execID.ID, dockercontainer.ResizeOptions{
					Height: ctrl.Rows,
					Width:  ctrl.Cols,
				})
				continue
			}
		}

		if _, werr := resp.Conn.Write(data); werr != nil {
			break
		}
	}

	// Best-effort: unblock the reader and wait briefly for it to finish.
	_ = resp.CloseWrite()

	select {
	case <-done:
	case <-time.After(time.Second):
	}

	return nil
}

func writeConsoleError(conn *websocket.Conn, msg string) {
	_ = conn.WriteMessage(websocket.TextMessage, []byte("\r\n\x1b[31m[clabstudio] "+msg+"\x1b[0m\r\n"))
}

func writeConsoleInfo(conn *websocket.Conn, msg string) {
	_ = conn.WriteMessage(websocket.TextMessage, []byte("\x1b[90m[clabstudio] "+msg+"\x1b[0m\r\n"))
}
