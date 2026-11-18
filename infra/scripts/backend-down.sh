#!/usr/bin/env bash
# Stop everything started by backend-up.sh. Data (Kafka, MinIO) is kept.
#   infra/scripts/backend-down.sh          # stop services, tunnels, containers
#   infra/scripts/backend-down.sh --quit   # ...and quit Docker Desktop
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN="$ROOT/.run"
# Docker/Homebrew CLIs may be missing from PATH (e.g. after a macOS update).
export PATH="/usr/local/bin:/opt/homebrew/bin:/Applications/Docker.app/Contents/Resources/bin:$HOME/.cargo/bin:$PATH"
export DOCKER_HOST="${DOCKER_HOST:-unix://$HOME/.docker/run/docker.sock}"

for pidfile in "$RUN"/*.pid; do
  [ -f "$pidfile" ] || continue
  pid=$(cat "$pidfile")
  if kill "$pid" 2>/dev/null; then
    echo "stopped $(basename "$pidfile" .pid)"
  fi
  rm -f "$pidfile"
done

if docker info >/dev/null 2>&1; then
  # Any sandboxes still running from submissions.
  ids=$(docker ps -q --filter "name=submission-")
  [ -n "$ids" ] && docker stop $ids >/dev/null && echo "stopped sandbox containers"
  docker compose -f "$ROOT/docker-compose.yml" stop
  if [ "${1:-}" = "--quit" ]; then
    osascript -e 'quit app "Docker"' && echo "quit Docker Desktop"
  fi
fi
