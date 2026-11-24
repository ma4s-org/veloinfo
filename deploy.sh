#!/usr/bin/env bash
set -euo pipefail

podman login ghcr.io

# --layers active les couches intermédiaires. Le Dockerfile utilise aussi des
# caches Cargo/npm persistants : seuls les crates modifiés sont recompilés.
podman build --layers -t martinhamel/veloinfo:latest .

podman tag martinhamel/veloinfo:latest martinhamel/veloinfo:temp
podman push martinhamel/veloinfo:temp ghcr.io/martinhamel/veloinfo:latest
podman rmi martinhamel/veloinfo:temp
