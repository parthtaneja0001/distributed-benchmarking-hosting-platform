#!/usr/bin/env bash
# Start the full benchmarking backend on this machine and expose it publicly
# through free Cloudflare quick tunnels.
#
#   infra/scripts/backend-up.sh                # start everything
#   VERCEL_TOKEN=... infra/scripts/backend-up.sh   # ...and point Vercel at it
#
# Stop with infra/scripts/backend-down.sh. Logs: .run/*.log
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN="$ROOT/.run"
mkdir -p "$RUN"
# Docker/Homebrew CLIs may be missing from PATH (e.g. after a macOS update).
export PATH="/usr/local/bin:/opt/homebrew/bin:/Applications/Docker.app/Contents/Resources/bin:$HOME/.cargo/bin:$PATH"
export DOCKER_HOST="${DOCKER_HOST:-unix://$HOME/.docker/run/docker.sock}"

log() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }

wait_for() { # wait_for <seconds> <description> <command...>
  local secs=$1 what=$2; shift 2
  for _ in $(seq 1 "$secs"); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  die "timed out waiting for $what"
}

# ---------------------------------------------------------------- 0. Tools
for tool in cargo go protoc cloudflared; do
  command -v "$tool" >/dev/null || die "$tool not found. Install it first (protoc: brew install protobuf; cloudflared: brew install cloudflared)"
done

# ---------------------------------------------------------------- 1. Docker
if ! docker info >/dev/null 2>&1; then
  log "Starting Docker Desktop..."
  open -a Docker
  wait_for 180 "Docker Desktop" docker info
fi

# ---------------------------------------------------------------- 2. Infra
log "Starting Redpanda (Kafka), Redis, S3 store..."
docker compose -f "$ROOT/docker-compose.yml" up -d redpanda redis s3
wait_for 120 "Redpanda" sh -c "docker exec redpanda rpk cluster health | grep -q 'Healthy:.*true'"
wait_for 60 "S3 store" curl -s -o /dev/null http://127.0.0.1:9000/
for t in submission.created sandbox.ready telemetry.raw orders.sent fills.actual; do
  docker exec redpanda rpk topic create "$t" >/dev/null 2>&1 || true
done

# ---------------------------------------------------------------- 3. Build
# Low priority + 2 jobs so building doesn't freeze the laptop.
build_rust() { (cd "$1" && nice -n 15 cargo build --release -j 2 --quiet); }
# Incremental: a no-op in seconds when nothing changed.
log "Building services (first run can take a while)..."
build_rust "$ROOT/services/telemetry"
build_rust "$ROOT/services/submission"
build_rust "$ROOT/services/bot-fleet"
(cd "$ROOT/services/leaderboard" && go build -o bin/leaderboard ./cmd)
(cd "$ROOT/services/sandbox-orchestrator" && go build -o bin/orchestrator ./cmd)

# ---------------------------------------------------------------- 4. Services
export KAFKA_BROKER=127.0.0.1:9092
export REDIS_URL=redis://127.0.0.1:6379 REDIS_ADDR=127.0.0.1:6379
export AWS_ACCESS_KEY_ID=minioadmin AWS_SECRET_ACCESS_KEY=minioadmin AWS_REGION=us-east-1

start() { # start <name> <cmd...>
  local name=$1; shift
  if [ -f "$RUN/$name.pid" ] && kill -0 "$(cat "$RUN/$name.pid")" 2>/dev/null; then
    log "$name already running"
    return
  fi
  nohup "$@" >"$RUN/$name.log" 2>&1 &
  echo $! >"$RUN/$name.pid"
  log "started $name (pid $!)"
}

B="$ROOT/services/bot-fleet/target/release"
start telemetry     "$ROOT/services/telemetry/target/release/telemetry"
start leaderboard   env REDIS_ADDR=127.0.0.1:6379 LEADERBOARD_HTTP_ADDR=127.0.0.1:8081 "$ROOT/services/leaderboard/bin/leaderboard"
start bot-worker-1  env PORT=50051 "$B/bot-worker"
start bot-worker-2  env PORT=50052 "$B/bot-worker"
start fleet-manager env BOT_WORKER_ADDRESSES=http://127.0.0.1:50051,http://127.0.0.1:50052 "$B/fleet-manager"
start orchestrator  env DEPLOY_MODE=docker "$ROOT/services/sandbox-orchestrator/bin/orchestrator"
start submission    env PORT=3000 "$ROOT/services/submission/target/release/submission"

wait_for 60 "submission service" curl -sf http://127.0.0.1:3000/healthz
wait_for 30 "leaderboard service" curl -sf http://127.0.0.1:8081/healthz

# ---------------------------------------------------------------- 5. Tunnels

tunnel_url() { grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' "$RUN/$1.log" 2>/dev/null | head -1; }

tunnel() { # tunnel <name> <local-url>  -> prints public URL
  local name=$1 url=$2 u
  # Reuse a running tunnel so the public URL (and Vercel's config) stays put,
  # but only if it still answers: Cloudflare deletes quick tunnels after a
  # long disconnect (e.g. laptop sleep) while cloudflared keeps retrying.
  if [ -f "$RUN/$name.pid" ] && kill -0 "$(cat "$RUN/$name.pid")" 2>/dev/null; then
    u=$(tunnel_url "$name")
    if [ -n "$u" ] && curl -sf -m 10 "$u/healthz" >/dev/null 2>&1; then echo "$u"; return 0; fi
    kill "$(cat "$RUN/$name.pid")" 2>/dev/null || true
    sleep 1
  fi
  rm -f "$RUN/$name.log"
  nohup cloudflared tunnel --no-autoupdate --url "$url" >"$RUN/$name.log" 2>&1 &
  echo $! >"$RUN/$name.pid"
  for _ in $(seq 1 60); do
    u=$(tunnel_url "$name")
    if [ -n "$u" ]; then echo "$u"; return 0; fi
    sleep 1
  done
  die "tunnel $name did not come up; see $RUN/$name.log"
}

log "Opening Cloudflare tunnels..."
UPLOAD_BASE=$(tunnel tunnel-upload http://127.0.0.1:3000)
WS_BASE=$(tunnel tunnel-leaderboard http://127.0.0.1:8081)
UPLOAD_URL="$UPLOAD_BASE/upload"
WS_URL="${WS_BASE/https:/wss:}/ws/leaderboard"
printf 'VITE_UPLOAD_URL=%s\nVITE_WS_URL=%s\n' "$UPLOAD_URL" "$WS_URL" >"$RUN/urls.env"

# New quick-tunnel hostnames can take a minute or two to resolve everywhere.
check_public() { # check_public <description> <url>
  for _ in $(seq 1 120); do
    if curl -sf -m 5 "$2" >/dev/null 2>&1; then log "$1 reachable"; return 0; fi
    sleep 1
  done
  printf '\033[1;33m!!\033[0m %s not reachable yet from this machine (DNS may still be propagating)\n' "$1"
}
check_public "public upload URL" "$UPLOAD_BASE/healthz"
check_public "public leaderboard URL" "$WS_BASE/healthz"

# ---------------------------------------------------------------- 6. Vercel
PROJECT_JSON="$ROOT/frontend/.vercel/project.json"
if [ -n "${VERCEL_TOKEN:-}" ] && [ -f "$PROJECT_JSON" ]; then
  log "Pointing Vercel at the new tunnel URLs and redeploying..."
  PROJECT_ID=$(python3 -c "import json;print(json.load(open('$PROJECT_JSON'))['projectId'])")
  ORG_ID=$(python3 -c "import json;print(json.load(open('$PROJECT_JSON'))['orgId'])")
  for kv in "VITE_UPLOAD_URL=$UPLOAD_URL" "VITE_WS_URL=$WS_URL"; do
    key=${kv%%=*}; val=${kv#*=}
    curl -sf -X POST "https://api.vercel.com/v10/projects/$PROJECT_ID/env?upsert=true&teamId=$ORG_ID" \
      -H "Authorization: Bearer $VERCEL_TOKEN" -H "Content-Type: application/json" \
      -d "{\"key\":\"$key\",\"value\":\"$val\",\"type\":\"plain\",\"target\":[\"production\",\"preview\"]}" >/dev/null \
      || die "failed to set $key on Vercel"
  done
  (cd "$ROOT/frontend" && VERCEL_ORG_ID=$ORG_ID VERCEL_PROJECT_ID=$PROJECT_ID \
    npx --yes vercel@latest deploy --prod --yes >"$RUN/vercel-deploy.log" 2>&1) \
    || die "Vercel deploy failed; see $RUN/vercel-deploy.log"
  log "Vercel redeployed."
else
  log "VERCEL_TOKEN not set: Vercel not updated. Set these in Vercel and redeploy:"
fi

echo
cat "$RUN/urls.env"
echo
log "Backend is up. Stop it with: infra/scripts/backend-down.sh"
