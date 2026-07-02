// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package api

import (
	"net/http"

	"github.com/srl-labs/containerlab/studio/engine"
)

// execRequest is the body for running a command on a node.
type execRequest struct {
	Cmd string `json:"cmd"`
}

func (h *Handler) execNode(w http.ResponseWriter, r *http.Request) {
	lab := r.PathValue("name")
	node := r.PathValue("node")

	var req execRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.Cmd == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "cmd is required"})
		return
	}

	res, err := h.Engine.Exec(r.Context(), lab, node, req.Cmd)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, res)
}

// lifecycleRequest is the body for a per-node lifecycle action.
type lifecycleRequest struct {
	Action string `json:"action"`
}

func (h *Handler) nodeLifecycle(w http.ResponseWriter, r *http.Request) {
	lab := r.PathValue("name")
	node := r.PathValue("node")

	var req lifecycleRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	valid := false

	for _, a := range engine.ValidActions() {
		if a == req.Action {
			valid = true
			break
		}
	}

	if !valid {
		writeJSON(w, http.StatusBadRequest,
			errorResponse{Error: "action must be one of start, stop, restart"})

		return
	}

	if err := h.Engine.NodeLifecycle(r.Context(), lab, node, req.Action); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": req.Action})
}

func (h *Handler) validateLab(w http.ResponseWriter, r *http.Request) {
	lab := r.PathValue("name")

	report, err := h.Engine.Validate(r.Context(), lab)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, report)
}
