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

	"github.com/srl-labs/containerlab/studio/ai"
	"github.com/srl-labs/containerlab/studio/engine"
)

// Handler holds the dependencies shared by all API handlers.
type Handler struct {
	Engine engine.Engine
	// AI is the Copilot agent. It may be nil when AI is not wired up.
	AI *ai.Agent
	// AuthToken, when non-empty, enables shared-secret authentication.
	AuthToken string
}

// New creates an API handler.
func New(eng engine.Engine, agent *ai.Agent, authToken string) *Handler {
	return &Handler{Engine: eng, AI: agent, AuthToken: authToken}
}

// RegisterRoutes mounts all REST routes on the provided mux (Go 1.22 pattern
// routing). WebSocket routes are registered by the server package.
func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/health", h.health)
	mux.HandleFunc("GET /api/capabilities", h.capabilities)
	mux.HandleFunc("POST /api/login", h.login)
	mux.HandleFunc("POST /api/logout", h.logout)
	mux.HandleFunc("GET /api/catalog", h.catalog)
	mux.HandleFunc("GET /api/templates", h.templates)
	mux.HandleFunc("POST /api/labs/from-template", h.labFromTemplate)

	mux.HandleFunc("GET /api/labs", h.listLabs)
	mux.HandleFunc("POST /api/labs", h.createLab)
	mux.HandleFunc("POST /api/labs/import", h.importLab)
	mux.HandleFunc("GET /api/labs/{name}", h.getLab)
	mux.HandleFunc("PUT /api/labs/{name}", h.saveLab)
	mux.HandleFunc("DELETE /api/labs/{name}", h.deleteLab)
	mux.HandleFunc("GET /api/labs/{name}/yaml", h.getLabYAML)
	mux.HandleFunc("POST /api/labs/{name}/yaml", h.updateLabYAML)
	mux.HandleFunc("GET /api/labs/{name}/status", h.labStatus)
	mux.HandleFunc("POST /api/labs/{name}/deploy", h.deployLab)
	mux.HandleFunc("POST /api/labs/{name}/destroy", h.destroyLab)
	mux.HandleFunc("POST /api/labs/{name}/save", h.saveConfigs)
	mux.HandleFunc("POST /api/labs/{name}/clone", h.cloneLab)
	mux.HandleFunc("POST /api/labs/{name}/rename", h.renameLab)

	mux.HandleFunc("POST /api/labs/{name}/nodes/{node}/exec", h.execNode)
	mux.HandleFunc("POST /api/labs/{name}/nodes/{node}/lifecycle", h.nodeLifecycle)
	mux.HandleFunc("POST /api/labs/{name}/nodes/{node}/impair", h.impairNode)
	mux.HandleFunc("POST /api/labs/{name}/nodes/{node}/capture", h.captureNode)
	mux.HandleFunc("POST /api/labs/{name}/validate", h.validateLab)
	mux.HandleFunc("POST /api/labs/{name}/iperf", h.iperfLab)
	mux.HandleFunc("POST /api/labs/{name}/configure", h.configureLab)

	mux.HandleFunc("POST /api/lint", h.lint)

	mux.HandleFunc("POST /api/ai/chat", h.aiChat)
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

// aiChat handles a Copilot chat turn, returning a natural-language reply and an
// optional proposed topology graph.
func (h *Handler) aiChat(w http.ResponseWriter, r *http.Request) {
	if h.AI == nil {
		writeJSON(w, http.StatusServiceUnavailable, errorResponse{Error: "copilot is not available"})
		return
	}

	var req ai.ChatRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	reply, err := h.AI.Chat(r.Context(), req)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, reply)
}

// decodeJSON reads and decodes a JSON request body into v.
func decodeJSON(r *http.Request, v any) error {
	defer r.Body.Close()

	dec := json.NewDecoder(r.Body)
	dec.DisallowUnknownFields()

	return dec.Decode(v)
}
