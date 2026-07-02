// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package api

import "net/http"

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
