// Command nodeagent is the test-container entrypoint. No args: sleep forever
// (keeps a --network=none container alive). -serve: TCP echo listener.
// -probe: TCP dial with a short retry window; exit 0 on connect, 1 on failure.
package main

import (
	"flag"
	"fmt"
	"io"
	"net"
	"os"
	"time"
)

func main() {
	serve := flag.String("serve", "", "TCP address to listen+echo on (e.g. :9000)")
	probe := flag.String("probe", "", "TCP ip:port to dial; exit 0 on connect")
	flag.Parse()
	switch {
	case *probe != "":
		os.Exit(runProbe(*probe))
	case *serve != "":
		runServe(*serve)
	default:
		for { // sleep forever; a bare select{} would deadlock-panic
			time.Sleep(time.Hour)
		}
	}
}

// runProbe retries briefly so probing right after the peer's detached serve
// exec stays reliable.
func runProbe(addr string) int {
	var err error
	for deadline := time.Now().Add(5 * time.Second); ; time.Sleep(200 * time.Millisecond) {
		var c net.Conn
		if c, err = net.DialTimeout("tcp", addr, time.Second); err == nil {
			c.Close()
			fmt.Println("probe ok:", addr)
			return 0
		}
		if time.Now().After(deadline) {
			fmt.Fprintln(os.Stderr, "probe failed:", err)
			return 1
		}
	}
}

func runServe(addr string) {
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		fmt.Fprintln(os.Stderr, "serve:", err)
		os.Exit(1)
	}
	for {
		c, err := ln.Accept()
		if err != nil {
			continue
		}
		go func() {
			defer c.Close()
			io.Copy(c, c)
		}()
	}
}
