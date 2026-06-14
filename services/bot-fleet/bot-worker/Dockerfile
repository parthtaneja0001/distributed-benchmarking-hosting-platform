FROM rust:latest AS builder
RUN apt-get update && apt-get install -y \
    cmake \
    libssl-dev \
    libsasl2-dev \
    libz-dev \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release -p bot-worker

FROM debian:stable-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    zlib1g \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/bot-worker /usr/local/bin/bot-worker
CMD ["bot-worker"]