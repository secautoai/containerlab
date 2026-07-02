// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package server

import (
	"net/http"

	clabstudioai "github.com/srl-labs/containerlab/studio/ai"
)

// registerAI registers the Copilot endpoints. The full agent endpoint is added
// in a later milestone.
func registerAI(mux *http.ServeMux, _ *clabstudioai.Agent) {
	mux.HandleFunc("POST /api/ai/chat", func(w http.ResponseWriter, _ *http.Request) {
		http.Error(w, `{"error":"ai chat not available"}`, http.StatusNotImplemented)
	})
}
