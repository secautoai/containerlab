// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package model

// KindInfo describes a containerlab node kind for the UI node palette.
type KindInfo struct {
	// Kind is the containerlab kind identifier used in topology files.
	Kind string `json:"kind"`
	// DisplayName is a human-friendly label for the palette.
	DisplayName string `json:"displayName"`
	// Vendor groups kinds by vendor/project in the palette.
	Vendor string `json:"vendor"`
	// DefaultImage is a sensible default container image for the kind.
	DefaultImage string `json:"defaultImage,omitempty"`
	// InterfacePattern documents how data interfaces are named (e.g. eth{n}).
	InterfacePattern string `json:"interfacePattern,omitempty"`
	// Icon is the UI icon identifier.
	Icon string `json:"icon"`
	// Container is true when the kind is a native container (vs a VM kind).
	Container bool `json:"container"`
	// Description is a short human-readable summary.
	Description string `json:"description,omitempty"`
}

// Catalog returns the curated list of node kinds offered in the UI palette.
// It focuses on the kinds most commonly used for labs and that run natively as
// containers (best experience without VM images), plus a generic linux host.
func Catalog() []KindInfo {
	return []KindInfo{
		{
			Kind:             "linux",
			DisplayName:      "Linux host",
			Vendor:           "Generic",
			DefaultImage:     "alpine:latest",
			InterfacePattern: "eth{n}",
			Icon:             "server",
			Container:        true,
			Description:      "Generic Linux container: client, server, or app host.",
		},
		{
			Kind:             "nokia_srlinux",
			DisplayName:      "Nokia SR Linux",
			Vendor:           "Nokia",
			DefaultImage:     "ghcr.io/nokia/srlinux:latest",
			InterfacePattern: "e1-{n}",
			Icon:             "router",
			Container:        true,
			Description:      "Nokia SR Linux containerized NOS.",
		},
		{
			Kind:             "arista_ceos",
			DisplayName:      "Arista cEOS",
			Vendor:           "Arista",
			DefaultImage:     "ceos:latest",
			InterfacePattern: "eth{n}",
			Icon:             "switch",
			Container:        true,
			Description:      "Arista cEOS containerized EOS (BYOI).",
		},
		{
			Kind:             "juniper_crpd",
			DisplayName:      "Juniper cRPD",
			Vendor:           "Juniper",
			DefaultImage:     "crpd:latest",
			InterfacePattern: "eth{n}",
			Icon:             "router",
			Container:        true,
			Description:      "Juniper containerized routing daemon (BYOI).",
		},
		{
			Kind:             "sonic-vs",
			DisplayName:      "SONiC (virtual switch)",
			Vendor:           "SONiC",
			DefaultImage:     "docker-sonic-vs:latest",
			InterfacePattern: "eth{n}",
			Icon:             "switch",
			Container:        true,
			Description:      "SONiC virtual switch.",
		},
		{
			Kind:             "cvx",
			DisplayName:      "NVIDIA Cumulus VX",
			Vendor:           "NVIDIA",
			DefaultImage:     "networkop/cx:5.3.0",
			InterfacePattern: "swp{n}",
			Icon:             "switch",
			Container:        true,
			Description:      "Cumulus Linux virtual appliance.",
		},
		{
			Kind:             "rare",
			DisplayName:      "RARE/freeRtr",
			Vendor:           "RARE",
			DefaultImage:     "ghcr.io/rare-freertr/freertr-containerlab:latest",
			InterfacePattern: "eth{n}",
			Icon:             "router",
			Container:        true,
			Description:      "RARE/freeRtr open-source router.",
		},
		{
			Kind:             "bridge",
			DisplayName:      "Linux bridge",
			Vendor:           "Generic",
			InterfacePattern: "eth{n}",
			Icon:             "cloud",
			Container:        false,
			Description:      "Pre-existing Linux bridge on the host.",
		},
		{
			Kind:             "host",
			DisplayName:      "Host",
			Vendor:           "Generic",
			InterfacePattern: "eth{n}",
			Icon:             "cloud",
			Container:        false,
			Description:      "The container host's network namespace.",
		},
	}
}

// DefaultImageForKind returns the curated default image for a kind, if any.
func DefaultImageForKind(kind string) string {
	for _, k := range Catalog() {
		if k.Kind == kind {
			return k.DefaultImage
		}
	}

	return ""
}

// InterfacePatternForKind returns the interface naming pattern for a kind.
// It defaults to "eth{n}" when the kind is unknown.
func InterfacePatternForKind(kind string) string {
	for _, k := range Catalog() {
		if k.Kind == kind && k.InterfacePattern != "" {
			return k.InterfacePattern
		}
	}

	return "eth{n}"
}
