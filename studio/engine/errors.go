// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package engine

import "errors"

// ErrRuntimeUnavailable is returned by lifecycle operations when no container
// runtime is reachable. The API layer maps this to a clear 503 response so the
// UI can keep design/AI features usable in "design-only" mode.
var ErrRuntimeUnavailable = errors.New("container runtime is unavailable")

// ErrNotFound is returned when a requested lab or node does not exist.
var ErrNotFound = errors.New("not found")
