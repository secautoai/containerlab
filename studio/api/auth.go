// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package api

import (
	"crypto/subtle"
	"net/http"
	"strings"
)

// authCookie is the name of the session cookie holding the shared secret.
const authCookie = "clabstudio_auth"

// authExemptExact are API paths always reachable without authentication so the
// SPA can load and the user can log in.
var authExemptExact = map[string]bool{
	"/api/login":        true,
	"/api/health":       true,
	"/api/capabilities": true,
}

// tokenValid reports whether the request carries the correct shared secret via
// the auth cookie or an Authorization: Bearer header (constant-time compare).
func tokenValid(r *http.Request, token string) bool {
	if c, err := r.Cookie(authCookie); err == nil && ctEqual(c.Value, token) {
		return true
	}

	if h := r.Header.Get("Authorization"); strings.HasPrefix(h, "Bearer ") {
		if ctEqual(strings.TrimPrefix(h, "Bearer "), token) {
			return true
		}
	}

	return false
}

func ctEqual(a, b string) bool {
	return subtle.ConstantTimeCompare([]byte(a), []byte(b)) == 1
}

// AuthMiddleware enforces the shared-secret token on API routes when a token is
// configured. When no token is set it is a no-op (open access). Non-API paths
// (the SPA and its assets) are always served so the login screen can load.
func (h *Handler) AuthMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if h.AuthToken == "" {
			next.ServeHTTP(w, r)
			return
		}

		path := r.URL.Path

		// Allow the SPA/static assets and explicitly-exempt API endpoints.
		if !strings.HasPrefix(path, "/api/") || authExemptExact[path] {
			next.ServeHTTP(w, r)
			return
		}

		if !tokenValid(r, h.AuthToken) {
			writeJSON(w, http.StatusUnauthorized, errorResponse{Error: "authentication required"})
			return
		}

		next.ServeHTTP(w, r)
	})
}

// loginRequest is the body for POST /api/login.
type loginRequest struct {
	Token string `json:"token"`
}

func (h *Handler) login(w http.ResponseWriter, r *http.Request) {
	if h.AuthToken == "" {
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
		return
	}

	var req loginRequest
	if err := decodeJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, errorResponse{Error: err.Error()})
		return
	}

	if !ctEqual(req.Token, h.AuthToken) {
		writeJSON(w, http.StatusUnauthorized, errorResponse{Error: "invalid token"})
		return
	}

	http.SetCookie(w, &http.Cookie{
		Name:     authCookie,
		Value:    h.AuthToken,
		Path:     "/",
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
	})

	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (h *Handler) logout(w http.ResponseWriter, _ *http.Request) {
	http.SetCookie(w, &http.Cookie{
		Name:     authCookie,
		Value:    "",
		Path:     "/",
		HttpOnly: true,
		MaxAge:   -1,
		SameSite: http.SameSiteLaxMode,
	})

	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}
