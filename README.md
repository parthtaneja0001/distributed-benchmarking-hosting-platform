# Distributed Benchmarking & Hosting Platform

IICPC Summer Hackathon 2026

A fully automated platform for stress‑testing contestant‑submitted trading engines.

Upload a matching engine → it is deployed inside an isolated sandbox → a distributed fleet of bots bombards it with realistic order flow → live latency, throughput, and correctness metrics appear on a real‑time leaderboard.

**Live demo:** https://snappy-slate-glixq6b-parth-tanejas-projects.vercel.app

> The frontend is hosted on Vercel; the backend runs on a self-hosted machine exposed through Cloudflare tunnels. When the backend is offline, the site shows clearly labelled demo data instead of live results.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Core Components](#core-components)
- [Technology Choices](#technology-choices)
- [Isolation Strategy](#isolation-strategy)
- [Scoring & Metrics](#scoring--metrics)
- [Setup & Deployment](#setup--deployment)
- [Environment Variables](#environment-variables)
- [Directory Structure](#directory-structure)
- [Testing Pipeline](#testing-pipeline)
- [Known Limitations & Future Work](#known-limitations--future-work)
- [Demo Video](#demo-video)
- [Team](#team)

---

## Architecture Overview

The platform follows an event‑driven microservices architecture.

All asynchronous communication happens through **Redpanda (Kafka‑compatible)** topics, while operational commands between the Fleet Manager and Bot Workers use **gRPC**.

┌──────────────┐
│ Contestant │
│ Upload │
└──────┬───────┘
│ multipart/form-data
▼
┌─────────────────┐ submission.created ┌────────────────────────┐
│ Submission │ ───────────────────────▶ │ Sandbox Orchestrator │
│ Service (Rust) │ │ (Go) │
│ • Axum HTTP │ │ • Kafka consumer │
│ • MinIO upload │ │ • Docker deployer │
│ • Lang detect │ └───────────┬────────────┘
└─────────────────┘ │ sandbox.ready
▼
gRPC StartTest
┌──────────────────────┐ ◄──────────────────────────────┐
│ Fleet Manager (Rust) │ │
│ • Kafka consumer │ │
│ • Load distribution │ │
└───────────┬──────────┘ │
│ │
▼ ▼
┌────────────────────────────────────────┐
│ Bot Workers (Rust) │
│ • gRPC server │
│ • Thousands of simulated traders │
│ • WebSocket load generation │
│ • Latency measurement │
└───────────┬────────────────────────────┘
│
│ telemetry.raw
│ orders.sent
│ fills.actual
▼
┌──────────────────────────────────────────────────────────────┐
│ Telemetry Ingester (Rust) │
│ • HDR Histogram (p50/p90/p99) │
│ • Shadow matching engine │
│ • Redis leaderboard cache │
│ • Historical metrics persistence │
└──────────────────────────┬───────────────────────────────────┘
│
▼
┌──────────────────────┐ WebSocket ┌──────────────────────┐
│ Leaderboard (Go) │ ◀───────────────────▶ │ Frontend (React) │
│ • Redis reader │ │ • Upload page │
│ • Score calculation │ │ • Live leaderboard │
└──────────────────────┘ └──────────────────────┘


### Data Flow

1. Contestant uploads a submission.
2. Submission Service stores the artifact in MinIO.
3. `submission.created` event is published.
4. Sandbox Orchestrator deploys the engine.
5. `sandbox.ready` event is published.
6. Fleet Manager distributes load across Bot Workers.
7. Bot Workers generate trading activity.
8. Telemetry Ingester computes metrics.
9. Leaderboard streams rankings to clients in real time.

---

## Core Components

| Service | Language | Key Technologies | Responsibility |
|----------|----------|------------------|----------------|
| Submission Service | Rust | Axum, rdkafka, MinIO | Accept uploads, detect language, store artifacts, emit events |
| Sandbox Orchestrator | Go | kafka-go, Docker SDK | Build and deploy isolated sandboxes |
| Fleet Manager | Rust | rdkafka, tonic | Coordinate benchmarking runs |
| Bot Worker | Rust | tonic, tokio-tungstenite, rdkafka | Generate load and collect latency metrics |
| Telemetry Ingester | Rust | HDR Histogram, Redis | Compute latency, throughput, and correctness |
| Leaderboard | Go | Redis, Gorilla WebSocket | Compute scores and push updates |
| Frontend | React | Vite, WebSocket | Submission portal and leaderboard |
| Infrastructure | Docker Compose | Redpanda, Redis, MinIO | Local development environment |

---

## Technology Choices

### Rust
Used for performance‑critical services:

- Memory safety without garbage collection
- Excellent async ecosystem
- Predictable latency
- High concurrency for thousands of simulated traders

### Go
Used for orchestration services:

- Simple concurrency model
- Excellent Docker ecosystem
- Fast development iteration

### Redpanda
Kafka‑compatible event streaming platform.

Benefits:
- Single binary deployment
- Lower operational complexity
- Low‑latency messaging
- Native Kafka API compatibility

### MinIO
S3‑compatible object storage.

Benefits:
- Shared artifact storage
- Cloud‑compatible architecture
- Simple local deployment

### Redis
Stores latest benchmark results.

Benefits:
- Sub‑millisecond access
- Ideal for live leaderboards
- Simple pub/sub patterns

### Docker
Current sandboxing layer.

Provides:
- Resource isolation
- Easy local development
- Consistent execution environments

### Firecracker (Planned)
Future production‑grade isolation.

Benefits:
- Hardware‑level virtualization
- Stronger security guarantees
- Fast microVM startup times

---

## Isolation Strategy

### Current: Docker
Each submission is:
1. Built inside a language‑specific builder image.
2. Packaged into a minimal runtime container.
3. Executed with strict resource limits.

Current limits:
- CPU: 1 core
- Memory: 512 MB
- Restricted networking
- Automatic cleanup after completion

### Future: Firecracker
The deployment layer is abstracted behind a `Deployer` interface.

This allows replacing Docker with Firecracker without affecting:
- Fleet Manager
- Bot Workers
- Telemetry Pipeline
- Leaderboard

---

## Scoring & Metrics

### Latency
Measured using HDR Histograms.

Reported metrics:
- p50
- p90
- p99

### Throughput
Acknowledged orders per second, measured over the run's active span using the
bots' own ack timestamps (not the ingester's processing time).

### Correctness
Many bots send orders concurrently, so the order in which the engine sees them
differs from the order on the `orders.sent` topic. Correctness therefore uses
checks that hold regardless of arrival order (`services/telemetry/src/validation.rs`):

- every reported fill belongs to an order that was actually sent,
- a limit order is never filled at a worse price than its limit,
- an order is never filled for more than its quantity,
- total fill volume agrees with what a reference price-time book expects.

`correctness = valid_fill_share × volume_agreement` (0–1). A correct engine scores
1.0; for example, an engine that re-rests fully filled orders (double fills) or
treats market orders as limits scores noticeably lower.

### Composite Score
```text
score = TPS × correctness − p99_us / 1000 − failures × 10
```

This rewards high throughput, high correctness and low tail latency. The scoring
function is pluggable (`services/leaderboard/internal/scoring`).

---

## Setup & Deployment

### Run the whole backend (one command)

Requires Docker Desktop, Rust, Go and `cloudflared` (`brew install cloudflared`).

```bash
infra/scripts/backend-up.sh
```

This starts Redpanda, Redis and an S3-compatible store in Docker, builds and starts
all services (submission :3000, leaderboard :8081, orchestrator in Docker mode,
fleet manager, two bot workers, telemetry), and opens two free Cloudflare quick
tunnels. It prints the public URLs:

```text
VITE_UPLOAD_URL=https://<random>.trycloudflare.com/upload
VITE_WS_URL=wss://<random>.trycloudflare.com/ws/leaderboard
```

Set those two variables on the Vercel project and redeploy, or run with
`VERCEL_TOKEN=<token> infra/scripts/backend-up.sh` to do it automatically
(uses `frontend/.vercel/project.json`). Tunnels are reused while they keep
running, so the URLs only change after a restart.

Stop everything (add `--quit` to also quit Docker Desktop):

```bash
infra/scripts/backend-down.sh
```

Logs are in `.run/*.log`. Submissions are benchmarked one at a time:
build in a locked-down container (no capabilities, 512 MB, 1 CPU, 256 pids,
loopback-only port), 30 s of load, then the container and image are removed.

### Frontend locally

```bash
cd frontend
npm install
npm run dev   # demo mode unless VITE_UPLOAD_URL / VITE_WS_URL are set
```

---

## Environment Variables

| Variable | Service | Default |
|----------|---------|---------|
| `AWS_ACCESS_KEY_ID` | Submission | `minioadmin` |
| `AWS_SECRET_ACCESS_KEY` | Submission | `minioadmin` |
| `AWS_REGION` | Submission | `us-east-1` |
| `DEPLOY_MODE` | Sandbox Orchestrator | `mock` |
| `BOT_WORKER_ADDRESSES` | Fleet Manager | `http://[::1]:50051` |
| `PORT` | Bot Worker | `50051` |

---

## Directory Structure

```text
distributed-benchmarking-hosting-platform/
├── frontend/
├── services/
│   ├── submission/
│   ├── sandbox-orchestrator/
│   ├── bot-fleet/
│   │   ├── fleet-manager/
│   │   ├── bot-worker/
│   │   └── Cargo.toml
│   ├── telemetry/
│   ├── leaderboard/
│   └── dummy-engine/
├── sample-engine/
├── infra/
│   ├── terraform/
│   ├── kubernetes/
│   └── scripts/
├── docker-compose.yml
└── README.md
```

---

## Testing Pipeline

1. Start all services.
2. Create the `submissions` bucket in MinIO.
3. Upload the sample engine through the frontend (or via `curl`).
4. Verify a `submission.created` event is produced.
5. Verify Sandbox Orchestrator deploys the engine.
6. Verify a `sandbox.ready` event is emitted.
7. Observe Bot Workers generating load.
8. Verify telemetry appears in Redis.
9. Observe leaderboard updates in the browser (or via WebSocket client).

---

## Known Limitations & Future Work

### Real‑Time Correctness Verification
The shadow order book currently consumes `orders.sent` and `fills.actual` independently.
Under heavy load, event ordering can occasionally create reconciliation races.

Future improvement:

- Windowed reconciliation
- End‑of‑test validation pass
- Deterministic correctness scoring

### Firecracker Integration
The deployment abstraction already supports Firecracker.
Requirements:

- Linux
- KVM support
- Bare‑metal or nested virtualization

### Dynamic Worker Discovery
Currently Fleet Manager uses a static worker list.

Future improvement:

- Redis‑backed service registry
- Automatic worker discovery
- Dynamic scaling

### Additional Language Support
Current profiles: Rust, Go, C++
Additional language profiles can be added with minimal changes.

---

