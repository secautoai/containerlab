// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package cmd

import (
	"os"

	"github.com/spf13/cobra"
	clabstudioengine "github.com/srl-labs/containerlab/studio/engine"
	clabstudioserver "github.com/srl-labs/containerlab/studio/server"
)

func studioCmd(o *Options) (*cobra.Command, error) {
	c := &cobra.Command{
		Use:   "studio",
		Short: "launch ClabStudio, a modern web UI + AI agent for building labs",
		Long: "ClabStudio serves a browser-based network simulator on top of " +
			"containerlab: a drag-and-drop topology canvas, lab lifecycle controls, " +
			"browser consoles and an AI Copilot.\nreference: https://containerlab.dev",
		RunE: func(cobraCmd *cobra.Command, _ []string) error {
			return studioFn(cobraCmd, o)
		},
	}

	c.Flags().StringVarP(
		&o.Studio.Address,
		"address",
		"a",
		o.Studio.Address,
		"web server listen address (host:port)",
	)
	c.Flags().StringVarP(
		&o.Studio.LabsDir,
		"labs-dir",
		"l",
		o.Studio.LabsDir,
		"directory where studio labs are stored (default ~/.clab/studio)",
	)
	c.Flags().StringVarP(
		&o.Studio.AIBaseURL,
		"ai-base-url",
		"",
		o.Studio.AIBaseURL,
		"base URL of an OpenAI-compatible API used by the Copilot",
	)
	c.Flags().StringVarP(
		&o.Studio.AIModel,
		"ai-model",
		"",
		o.Studio.AIModel,
		"chat model used by the Copilot",
	)
	c.Flags().StringVarP(
		&o.Studio.AuthToken,
		"auth-token",
		"",
		o.Studio.AuthToken,
		"shared secret to require login (also via CLAB_STUDIO_AUTH_TOKEN); empty = open access",
	)

	return c, nil
}

func studioFn(cobraCmd *cobra.Command, o *Options) error {
	// The AI API key is read from the environment to avoid leaking it via flags.
	if o.Studio.AIAPIKey == "" {
		o.Studio.AIAPIKey = os.Getenv("CLAB_STUDIO_AI_API_KEY")
		if o.Studio.AIAPIKey == "" {
			o.Studio.AIAPIKey = os.Getenv("OPENAI_API_KEY")
		}
	}

	// The auth token is read from the environment to avoid leaking it via flags.
	if o.Studio.AuthToken == "" {
		o.Studio.AuthToken = os.Getenv("CLAB_STUDIO_AUTH_TOKEN")
	}

	eng, err := clabstudioengine.NewClabEngine(clabstudioengine.Config{
		LabsDir: o.Studio.LabsDir,
		Runtime: o.Global.Runtime,
		Timeout: o.Global.Timeout,
		Owner:   getStudioOwner(o),
	})
	if err != nil {
		return err
	}

	return clabstudioserver.Run(cobraCmd.Context(), clabstudioserver.Config{
		Address:   o.Studio.Address,
		Engine:    eng,
		AIBaseURL: o.Studio.AIBaseURL,
		AIModel:   o.Studio.AIModel,
		AIAPIKey:  o.Studio.AIAPIKey,
		AuthToken: o.Studio.AuthToken,
	})
}

func getStudioOwner(_ *Options) string {
	if owner := os.Getenv("CLAB_OWNER"); owner != "" {
		return owner
	}

	if owner := os.Getenv("SUDO_USER"); owner != "" {
		return owner
	}

	return os.Getenv("USER")
}
