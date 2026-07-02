// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package api implements ClabStudio's HTTP + WebSocket handlers on top of the
// engine.Engine abstraction.
package api

import (
	"encoding/json"
	"errors"
	"net/http"

	"github.com/srl-labs/containerlab/studio/engine"
)

// Handler holds the dependencies shared by all API handlers.
type Handler struct {
	Engine engine.Engine
	// AI is the optional Copilot agent. It may be nil when AI is not configured.
	AI Copilot
}

// Copilot is the minimal interface the API needs from the AI agent. It is kept
// here (rather than importing the ai package) to avoid an import cycle and to
// allow the agent to be nil/stubbed.
type Copilot interface {
	// Available reports whether a real LLM backend is configured.
	Available() bool
}

// New creates an API handler.
func New(eng engine.Engine, ai Copilot) *Handler {
	return &Handler{Engine: eng, AI: ai}
}

// RegisterRoutes mounts all REST routes on the provided mux (Go 1.22 pattern
// routing). WebSocket routes are registered by the server package.
func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/health", h.health)
	mux.HandleFunc("GET /api/capabilities", h.capabilities)
	mux.HandleFunc("GET /api/catalog", h.catalog)

	mux.HandleFunc("GET /api/labs", h.listLabs)
	mux.HandleFunc("POST /api/labs", h.createLab)
	mux.HandleFunc("GET /api/labs/{name}", h.getLab)
	mux.HandleFunc("PUT /api/labs/{name}", h.saveLab)
	mux.HandleFunc("DELETE /api/labs/{name}", h.deleteLab)
	mux.HandleFunc("GET /api/labs/{name}/yaml", h.getLabYAML)
	mux.HandleFunc("GET /api/labs/{name}/status", h.labStatus)
	mux.HandleFunc("POST /api/labs/{name}/deploy", h.deployLab)
	mux.HandleFunc("POST /api/labs/{name}/destroy", h.destroyLab)

	mux.HandleFunc("POST /api/labs/{name}/nodes/{node}/exec", h.execNode)
}

// writeJSON encodes v as JSON with the given status code.
func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)

	if v != nil {
		_ = json.NewEncoder(w).Encode(v)
	}
}

// errorResponse is the standard error body.
type errorResponse struct {
	Error string `json:"error"`
}

// writeError maps an error to an appropriate HTTP status + JSON body.
func writeError(w http.ResponseWriter, err error) {
	status := http.StatusInternalServerError

	switch {
	case errors.Is(err, engine.ErrRuntimeUnavailable):
		status = http.StatusServiceUnavailable
	case errors.Is(err, engine.ErrNotFound):
		status = http.StatusNotFound
	}

	writeJSON(w, status, errorResponse{Error: err.Error()})
}

// decodeJSON reads and decodes a JSON request body into v.
func decodeJSON(r *http.Request, v any) error {
	defer r.Body.Close()

	dec := json.NewDecoder(r.Body)
	dec.DisallowUnknownFields()

	return dec.Decode(v)
}
