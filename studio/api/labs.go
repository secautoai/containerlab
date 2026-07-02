// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package api

import (
	"fmt"
	"net/http"

	"github.com/srl-labs/containerlab/studio/ai"
	"github.com/srl-labs/containerlab/studio/model"
)

func (h *Handler) health(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (h *Handler) capabilities(w http.ResponseWriter, r *http.Request) {
	caps := h.Engine.Capabilities(r.Context())

	resp := map[string]any{
		"runtimeAvailable": caps.RuntimeAvailable,
		"runtime":          caps.Runtime,
		"reason":           caps.Reason,
		"aiAvailable":      h.AI != nil && h.AI.Available(),
	}

	writeJSON(w, http.StatusOK, resp)
}

func (h *Handler) catalog(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, model.Catalog())
}

func (h *Handler) templates(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, ai.Templates())
}

// fromTemplateRequest selects a template and target lab name.
type fromTemplateRequest struct {
	TemplateID string `json:"templateId"`
	Name       string `json:"name,omitempty"`
}

// labFromTemplate instantiates a starter template into a new saved lab.
func (h *Handler) labFromTemplate(w http.ResponseWriter, r *http.Request) {
	var req fromTemplateRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.TemplateID == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "templateId is required"})
		return
	}

	res, err := ai.BuildTemplate(req.TemplateID, req.Name)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if err := h.Engine.SaveLab(r.Context(), res.Graph); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusCreated, res.Graph)
}

func (h *Handler) listLabs(w http.ResponseWriter, r *http.Request) {
	labs, err := h.Engine.ListLabs(r.Context())
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, labs)
}

// createLabRequest is the body for creating a new lab.
type createLabRequest struct {
	Name string `json:"name"`
}

func (h *Handler) createLab(w http.ResponseWriter, r *http.Request) {
	var req createLabRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.Name == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "name is required"})
		return
	}

	g := &model.Graph{Name: req.Name, Nodes: []*model.Node{}, Links: []*model.Link{}}
	if err := h.Engine.SaveLab(r.Context(), g); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusCreated, g)
}

// importRequest carries a containerlab topology YAML to import as a new lab.
type importRequest struct {
	Name string `json:"name,omitempty"`
	YAML string `json:"yaml"`
}

// importLab parses a pasted/uploaded containerlab topology into a lab, applying
// auto-layout so it renders sensibly on the canvas.
func (h *Handler) importLab(w http.ResponseWriter, r *http.Request) {
	var req importRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.YAML == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "yaml is required"})
		return
	}

	g, err := model.ClabYAMLToGraph([]byte(req.YAML))
	if err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.Name != "" {
		g.Name = req.Name
	}

	if g.Name == "" {
		writeJSON(w, http.StatusBadRequest,
			errorResponse{Error: "topology has no name; provide one"})

		return
	}

	model.AutoLayout(g)

	if err := h.Engine.SaveLab(r.Context(), g); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusCreated, g)
}

// nameRequest carries a target lab name for clone/rename.
type nameRequest struct {
	Name string `json:"name"`
}

func (h *Handler) cloneLab(w http.ResponseWriter, r *http.Request) {
	src := r.PathValue("name")

	var req nameRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.Name == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "name is required"})
		return
	}

	if err := h.Engine.CloneLab(r.Context(), src, req.Name); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusCreated, map[string]string{"name": req.Name})
}

func (h *Handler) renameLab(w http.ResponseWriter, r *http.Request) {
	old := r.PathValue("name")

	var req nameRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if req.Name == "" {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: "name is required"})
		return
	}

	if err := h.Engine.RenameLab(r.Context(), old, req.Name); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"name": req.Name})
}

func (h *Handler) saveConfigs(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	if err := h.Engine.SaveConfigs(r.Context(), name); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "configs saved"})
}

// lint runs pre-flight design checks on a posted topology graph.
func (h *Handler) lint(w http.ResponseWriter, r *http.Request) {
	var g model.Graph
	if err := decodeJSON(r, &g); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	writeJSON(w, http.StatusOK, model.Lint(&g))
}

func (h *Handler) getLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	g, err := h.Engine.GetLab(r.Context(), name)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, g)
}

func (h *Handler) saveLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	var g model.Graph
	if err := decodeJSON(r, &g); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	// The URL path is authoritative for the lab name.
	if g.Name == "" {
		g.Name = name
	}

	if g.Name != name {
		writeJSON(w, http.StatusBadRequest,
			errorResponse{Error: fmt.Sprintf("body name %q does not match path %q", g.Name, name)})

		return
	}

	if err := h.Engine.SaveLab(r.Context(), &g); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, &g)
}

func (h *Handler) deleteLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	if err := h.Engine.DeleteLab(r.Context(), name); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "deleted"})
}

func (h *Handler) getLabYAML(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	data, err := h.Engine.RenderYAML(r.Context(), name)
	if err != nil {
		writeError(w, err)
		return
	}

	w.Header().Set("Content-Type", "application/x-yaml")
	w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", name+".clab.yml"))
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(data)
}

func (h *Handler) labStatus(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	st, err := h.Engine.Status(r.Context(), name)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, st)
}

// configureRequest selects the routing protocol for auto-configuration.
type configureRequest struct {
	Protocol string `json:"protocol"`
}

// configureResponse returns the updated graph and the addressing plan.
type configureResponse struct {
	Graph *model.Graph    `json:"graph"`
	Plan  *ai.AddressPlan `json:"plan"`
}

// configureLab assigns IP addressing and generates per-node config for a lab,
// persisting the result. Linux/FRR nodes receive exec-based configuration.
func (h *Handler) configureLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	var req configureRequest
	_ = decodeJSON(r, &req) // body optional; defaults to addressing only

	if req.Protocol == "" {
		req.Protocol = "none"
	}

	g, err := h.Engine.GetLab(r.Context(), name)
	if err != nil {
		writeError(w, err)
		return
	}

	plan, err := ai.AutoConfigure(g, req.Protocol)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if err := h.Engine.SaveLab(r.Context(), g); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, configureResponse{Graph: g, Plan: plan})
}

func (h *Handler) deployLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	st, err := h.Engine.Deploy(r.Context(), name)
	if err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, st)
}

// destroyRequest is the optional body for destroy.
type destroyRequest struct {
	Cleanup bool `json:"cleanup"`
}

func (h *Handler) destroyLab(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")

	var req destroyRequest
	// body is optional; ignore decode errors on empty body
	_ = decodeJSON(r, &req)

	if err := h.Engine.Destroy(r.Context(), name, req.Cleanup); err != nil {
		writeError(w, err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "destroyed"})
}
