// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package frontend embeds the built ClabStudio single-page application.
//
// The contents of the dist/ directory are produced by the Vite build
// (`make studio-frontend`). A minimal placeholder is committed so that
// `go build ./...` always succeeds even before the frontend is built.
package frontend

import (
	"embed"
	"io/fs"
)

//go:embed all:dist
var dist embed.FS

// DistFS returns the embedded, built frontend rooted at the dist directory.
func DistFS() (fs.FS, error) {
	return fs.Sub(dist, "dist")
}
