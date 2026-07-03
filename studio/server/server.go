// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// Package server wires ClabStudio's HTTP server: REST API, WebSocket endpoints
// and the embedded single-page application.
package server

import (
	"context"
	"errors"
	"net/http"
	"time"

	"github.com/charmbracelet/log"
	clabstudioai "github.com/srl-labs/containerlab/studio/ai"
	clabstudioapi "github.com/srl-labs/containerlab/studio/api"
	clabstudioengine "github.com/srl-labs/containerlab/studio/engine"
)

// Config configures the ClabStudio server.
type Config struct {
	Address   string
	Engine    clabstudioengine.Engine
	AIBaseURL string
	AIModel   string
	AIAPIKey  string
	AuthToken string
}

// Run starts the ClabStudio HTTP server and blocks until the context is
// canceled, at which point it shuts down gracefully.
func Run(ctx context.Context, cfg Config) error {
	if cfg.Address == "" {
		cfg.Address = "0.0.0.0:8080"
	}

	agent := clabstudioai.NewAgent(clabstudioai.Config{
		BaseURL: cfg.AIBaseURL,
		Model:   cfg.AIModel,
		APIKey:  cfg.AIAPIKey,
		Engine:  cfg.Engine,
	})

	handler := clabstudioapi.New(cfg.Engine, agent, cfg.AuthToken)

	mux := http.NewServeMux()
	handler.RegisterRoutes(mux)

	// WebSocket console endpoint (REST + AI chat are on the api handler).
	registerConsole(mux, cfg.Engine)

	spa, err := spaHandler()
	if err != nil {
		return err
	}

	mux.Handle("/", spa)

	srv := &http.Server{
		Addr:              cfg.Address,
		Handler:           recoverMiddleware(logMiddleware(handler.AuthMiddleware(mux))),
		ReadHeaderTimeout: 10 * time.Second,
	}

	errCh := make(chan error, 1)

	go func() {
		caps := cfg.Engine.Capabilities(ctx)
		log.Info("ClabStudio starting",
			"address", cfg.Address,
			"runtime", caps.Runtime,
			"runtimeAvailable", caps.RuntimeAvailable,
			"aiAvailable", agent.Available(),
		)
		log.Infof("Open ClabStudio at http://%s", cfg.Address)

		if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errCh <- err
		}
	}()

	select {
	case <-ctx.Done():
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()

		return srv.Shutdown(shutdownCtx)
	case err := <-errCh:
		return err
	}
}
