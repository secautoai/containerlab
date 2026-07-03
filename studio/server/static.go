// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package server

import (
	"io/fs"
	"net/http"
	"path"
	"strings"

	clabstudiofrontend "github.com/srl-labs/containerlab/studio/frontend"
)

// spaHandler serves the embedded single-page application. Requests for real
// files (assets) are served directly; anything else falls back to index.html so
// client-side routing works.
func spaHandler() (http.Handler, error) {
	distFS, err := clabstudiofrontend.DistFS()
	if err != nil {
		return nil, err
	}

	fileServer := http.FileServer(http.FS(distFS))

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		reqPath := strings.TrimPrefix(path.Clean(r.URL.Path), "/")
		if reqPath == "" {
			reqPath = "index.html"
		}

		if _, err := fs.Stat(distFS, reqPath); err != nil {
			// Not a real file -> serve the SPA entrypoint.
			r2 := new(http.Request)
			*r2 = *r
			r2.URL.Path = "/"
			serveIndex(w, r2, distFS)

			return
		}

		fileServer.ServeHTTP(w, r)
	}), nil
}

// serveIndex writes the SPA index.html for client-routed paths.
func serveIndex(w http.ResponseWriter, _ *http.Request, distFS fs.FS) {
	data, err := fs.ReadFile(distFS, "index.html")
	if err != nil {
		http.Error(w, "index.html not found", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(data)
}
