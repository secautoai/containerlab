# studio

## Description

The `studio` command launches **ClabStudio**, a modern, browser-based network
simulator built on top of containerlab. It is inspired by tools like EVE-NG and
NetPilot and adds three things on top of the containerlab engine:

- **A modern topology canvas** — drag-and-drop nodes from a vendor-grouped
  palette, draw links, arrange your layout, and save it as a standard
  `*.clab.yml` topology file.
- **Lab lifecycle & browser consoles** — deploy/destroy labs, watch node status,
  and open an interactive terminal into any running node right in the browser.
- **An AI Copilot ("vibe labbing")** — describe a network in plain English and
  the Copilot designs a topology you can review, apply to the canvas, and deploy.

ClabStudio is a single, self-contained web server: it serves an embedded
single-page application and a REST + WebSocket API, reusing the containerlab
`core` engine under the hood. Labs are stored on disk as normal containerlab
topology files, so anything you build in ClabStudio also works with the regular
`containerlab` CLI (and vice-versa).

## Usage

```
containerlab studio [flags]
```

## Flags

### `--address | -a`

Listen address for the web server in `host:port` form. Defaults to
`0.0.0.0:8080`.

### `--labs-dir | -l`

Directory where ClabStudio stores labs. Each lab lives in
`<labs-dir>/<name>/<name>.clab.yml`. Defaults to `~/.clab/studio`.

### `--ai-base-url`

Base URL of an OpenAI-compatible chat completions API used by the Copilot.
Defaults to `https://api.openai.com/v1`.

### `--ai-model`

Chat model used by the Copilot. Defaults to `gpt-4o-mini`.

The API key is read from the environment (never a flag) via
`CLAB_STUDIO_AI_API_KEY` (or `OPENAI_API_KEY`). When no key is configured, the
Copilot still works using a built-in **offline topology generator** that
understands common intents (linear, ring, star, mesh, leaf-spine/clos, triangle;
kinds such as SR Linux, Arista, Juniper, FRR/Linux; and protocol hints).

## Examples

Start ClabStudio on the default address using the default labs directory:

```bash
sudo containerlab studio
```

Then open the printed URL (e.g. `http://<host>:8080`) in your browser.

Use a custom port and labs directory:

```bash
sudo containerlab studio --address 0.0.0.0:9000 --labs-dir /opt/clab-labs
```

Enable the LLM-backed Copilot with an OpenAI-compatible endpoint:

```bash
export CLAB_STUDIO_AI_API_KEY=sk-...
sudo containerlab studio --ai-base-url https://api.openai.com/v1 --ai-model gpt-4o-mini
```

## Notes

- Deploying labs and opening consoles requires a working container runtime
  (Docker) and typically root privileges, exactly like the rest of
  containerlab. If the runtime is unavailable, ClabStudio runs in **design-only
  mode**: you can still build topologies, export YAML, and use the Copilot, but
  lifecycle actions are disabled with a clear message.
- Because labs are plain `*.clab.yml` files, you can `git`-version them, edit
  them by hand, or manage them with the `containerlab` CLI.
- Node canvas positions and icons are persisted inside node `labels`
  (`graph-posX`, `graph-posY`, `graph-icon`) so a saved lab round-trips through
  the canvas without losing its layout.

## Feature overview

| Area          | What you can do                                                            |
| ------------- | ------------------------------------------------------------------------- |
| Canvas        | Add/remove nodes, draw/remove links, drag layout, edit node properties    |
| Labs          | Create, templates, import, open, save, clone, rename, delete, export        |
| Lifecycle     | Deploy, destroy, save running configs                                       |
| Status        | Live per-node state and management IPs                                     |
| Console       | Browser terminal (xterm.js) into any running node                          |
| Check         | Pre-flight topology linter (errors/warnings) — gates deploy                 |
| Validation    | End-to-end reachability (ping matrix) with a pass/fail report             |
| Node ops      | Per-node start / stop / restart                                            |
| Impairments   | Per-interface netem: delay, jitter, loss, rate, corruption                 |
| YAML editor   | View/edit the raw `*.clab.yml` in-app and apply to the canvas               |
| Auto-config   | Assign IP addressing (/30 links, /32 loopbacks) + OSPF/BGP for FRR/Linux   |
| Copilot       | Generate topologies + conversationally edit (add/connect/remove) + config  |

## REST API

ClabStudio exposes a small JSON API (used by the SPA, also usable directly):

| Method & path                                   | Description                    |
| ----------------------------------------------- | ------------------------------ |
| `GET /api/health`                               | Health check                   |
| `GET /api/capabilities`                         | Runtime & AI availability      |
| `GET /api/catalog`                              | Node kind palette              |
| `GET /api/templates`                            | Quick-start topology catalog   |
| `POST /api/labs/from-template`                  | Create a lab from a template   |
| `GET /api/labs`                                 | List labs                      |
| `POST /api/labs`                                | Create a lab                   |
| `POST /api/labs/import`                         | Import a topology YAML          |
| `GET/PUT/DELETE /api/labs/{name}`               | Read / save / delete a lab     |
| `POST /api/labs/{name}/save`                    | Save running configs            |
| `POST /api/labs/{name}/clone`                   | Clone a lab                     |
| `POST /api/labs/{name}/rename`                  | Rename a lab                    |
| `GET /api/labs/{name}/yaml`                     | Download the topology YAML     |
| `POST /api/labs/{name}/yaml`                    | Replace topology from raw YAML  |
| `GET /api/labs/{name}/status`                   | Runtime status                 |
| `POST /api/labs/{name}/deploy` \| `/destroy`    | Lifecycle                      |
| `POST /api/lint`                                | Pre-flight topology check      |
| `POST /api/labs/{name}/validate`                | Reachability report            |
| `POST /api/labs/{name}/configure`               | Auto IP addressing + config    |
| `POST /api/labs/{name}/nodes/{node}/exec`       | Run a command on a node        |
| `POST /api/labs/{name}/nodes/{node}/lifecycle`  | start / stop / restart a node  |
| `POST /api/labs/{name}/nodes/{node}/impair`     | set/clear netem impairments    |
| `GET  /api/labs/{name}/nodes/{node}/console`    | WebSocket console (TTY)        |
| `POST /api/ai/chat`                             | Copilot chat turn              |
