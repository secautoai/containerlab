// Copyright 2025
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"
)

// Provider is a minimal client for an OpenAI-compatible chat completions API.
type Provider struct {
	baseURL string
	model   string
	apiKey  string
	client  *http.Client
}

// NewProvider builds a chat provider. baseURL defaults to the OpenAI API.
func NewProvider(baseURL, model, apiKey string) *Provider {
	if baseURL == "" {
		baseURL = "https://api.openai.com/v1"
	}

	baseURL = strings.TrimRight(baseURL, "/")

	if model == "" {
		model = "gpt-4o-mini"
	}

	return &Provider{
		baseURL: baseURL,
		model:   model,
		apiKey:  apiKey,
		client:  &http.Client{Timeout: 90 * time.Second},
	}
}

// ChatMessage is a single message in a chat conversation.
type ChatMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

type chatRequest struct {
	Model    string        `json:"model"`
	Messages []ChatMessage `json:"messages"`
	Stream   bool          `json:"stream"`
}

type chatResponse struct {
	Choices []struct {
		Message ChatMessage `json:"message"`
	} `json:"choices"`
	Error *struct {
		Message string `json:"message"`
	} `json:"error"`
}

// Complete sends a non-streaming chat completion request and returns the
// assistant message content.
func (p *Provider) Complete(ctx context.Context, messages []ChatMessage) (string, error) {
	body, err := json.Marshal(chatRequest{Model: p.model, Messages: messages})
	if err != nil {
		return "", err
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost,
		p.baseURL+"/chat/completions", bytes.NewReader(body))
	if err != nil {
		return "", err
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(req)
	if err != nil {
		return "", err
	}

	defer resp.Body.Close()

	var cr chatResponse
	if err := json.NewDecoder(resp.Body).Decode(&cr); err != nil {
		return "", fmt.Errorf("decode chat response: %w", err)
	}

	if cr.Error != nil {
		return "", fmt.Errorf("ai provider error: %s", cr.Error.Message)
	}

	if len(cr.Choices) == 0 {
		return "", fmt.Errorf("ai provider returned no choices")
	}

	return cr.Choices[0].Message.Content, nil
}
