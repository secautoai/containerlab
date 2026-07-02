// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package server

import (
	"net/http"

	clabstudioengine "github.com/srl-labs/containerlab/studio/engine"
)

// registerConsole registers the browser-console WebSocket endpoint.
// The full interactive implementation is added in a later milestone; until then
// the endpoint reports that the console is not yet available.
func registerConsole(mux *http.ServeMux, _ clabstudioengine.Engine) {
	mux.HandleFunc("GET /api/labs/{name}/nodes/{node}/console", func(w http.ResponseWriter, _ *http.Request) {
		http.Error(w, `{"error":"console not available"}`, http.StatusNotImplemented)
	})
}
