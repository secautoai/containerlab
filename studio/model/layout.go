// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

import "math"

// Layout tuning constants for AutoLayout.
const (
	layoutGap     = 200.0 // spacing between grid cells (px)
	layoutOriginX = 120.0
	layoutOriginY = 100.0
)

// AutoLayout assigns grid positions to nodes that do not already have one
// (position == {0,0}), so imported topologies render sensibly on the canvas.
// Nodes that already carry a non-zero position are left untouched.
func AutoLayout(g *Graph) {
	if g == nil || len(g.Nodes) == 0 {
		return
	}

	// Collect nodes needing a position.
	var pending []*Node

	for _, n := range g.Nodes {
		if n.Position.X == 0 && n.Position.Y == 0 {
			pending = append(pending, n)
		}
	}

	if len(pending) == 0 {
		return
	}

	// Arrange pending nodes in a near-square grid.
	cols := int(math.Ceil(math.Sqrt(float64(len(pending)))))
	if cols < 1 {
		cols = 1
	}

	for i, n := range pending {
		row := i / cols
		col := i % cols
		n.Position = Position{
			X: layoutOriginX + float64(col)*layoutGap,
			Y: layoutOriginY + float64(row)*layoutGap,
		}
	}
}
