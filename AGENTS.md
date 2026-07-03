# Agents

Containerlab is a CLI tool for building and managing labs with containerized network devices. It provides a simple and efficient way to create both small and large network topologies using Docker containers.

## Repository structure

The repository is organized as follows:

- `.github/workflows`: Contains GitHub Actions workflows for CI/CD.
- `bin`: Directory where the built containerlab binary is placed.
- `cmd`: Contains the main command-line interface code for containerlab written with Cobra framework.
- `core`: Contains the core logic of containerlab, including lab management, topology parsing, and other functionalities.
- `docs`: Contains the documentation for containerlab, built with the Zensical static site generator.
- `nodes`: Contains the definitions and implementations of different types of network nodes that can be used in labs.
- `links`: Contains the logic for managing links between nodes in the lab.
- `tests`: Contains unit and integration tests for containerlab.
- `runtime`: Contains the runtime (docker and podman) logic for managing the lifecycle of labs and nodes.

## Documentation

Documentation is built with the Zensical static site generator (the successor to mkdocs-material, by the same team) and is present in the `docs` directory. Zensical is installed via the `docs` uv dependency group and still reads the existing `mkdocs.yml` configuration. To serve the documentation locally, use:

```bash
make serve-docs
```

To cleanup all started labs, use:

```bash
clab des -a -c -y
```

## Formatting

To format the code, use:

```bash
make format
```

## Building

The build and test automation is done in Makefile.

To build the clab binary, use:

```bash
make build
```

When building, the containerlab binary (alias `clab`) is placed in the `bin` directory.

## Testing

The unit tests are implemented with Go's built-in testing framework. To run the tests, use the following command:

```bash
make test
```

The integration tests are implemented with Robot Framework. To run the tests, use the following command:

```bash
CLAB_BIN=$(pwd)/bin/containerlab ./tests/rf-run.sh docker tests/<path to robot file>
```

When integration tests are run, the lab is destroyed automatically after the test ends.

To run lab deployment with debug logs, use `-d` flag.

## Cursor Cloud specific instructions

Standard build/test/lint/docs commands are documented in the sections above; the notes below cover only non-obvious, environment-specific caveats for this cloud VM.

### Docker must be started manually each session
Docker is installed but there is no init system, so the daemon is not running at
startup and must be started once per session before building/deploying labs or
running tests that touch the runtime:

```bash
sudo dockerd    # run in a background tmux session
```

Docker is intentionally configured for this VM: `fuse-overlayfs` storage driver
with the containerd snapshotter disabled (`/etc/docker/daemon.json`) and
`iptables-legacy`. Do not change these — the microVM kernel cannot use the
default `overlay2` driver or nftables.

### Docker socket permissions
`make test` and the `containerlab` CLI need access to `/var/run/docker.sock`.
The `ubuntu` user is in the `docker` group, so a fresh login shell has access.
If you see `permission denied ... docker.sock` in an already-open shell, run the
command through `sg docker -c '<cmd>'` or as root (the built binary is setuid
root, so `sudo ./bin/containerlab ...` always works).

### `clab deploy` cannot run on this kernel (hard limitation)
The Firecracker microVM kernel is built without loadable-module support
(`CONFIG_MODULES=n`), so `/proc/modules` does not exist. `loadKernelModules()`
in `core/config.go` reads that file during every deploy and returns a fatal
error (`open /proc/modules: no such file or directory`). This is a kernel
limitation, not a code bug, and it cannot be worked around from userspace
(bind-mount, fresh procfs, overlay, and `proot` were all tried; `proot` also
breaks nsfs/netns paths). Real lab deployment requires a host kernel with
`CONFIG_MODULES=y`.

Non-deploy commands work normally, e.g. topology parsing/validation and graph
generation:

```bash
./bin/containerlab graph --offline --dot -t <topo>.clab.yml   # or --mermaid
./bin/containerlab inspect -t <topo>.clab.yml
```

### Unit tests
`make test` passes except `TestIsKernelModuleLoaded` (in `utils`), which fails
only because `/proc/modules` is absent (same kernel limitation as above). All
other packages pass once Docker is running and the shell has docker-group access.

### Lint
`golangci-lint` v2.12.2 is installed to `$(go env GOPATH)/bin` (on `PATH`); run
`make lint`. The `make clint`/`make format` targets run linters inside Docker
with `-it` and will fail in non-interactive shells — prefer `make lint`.
