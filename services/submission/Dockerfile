FROM rust:latest AS builder
RUN apt-get update && apt-get install -y \
    cmake \
    libssl-dev \
    libsasl2-dev \
    libz-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:stable-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    zlib1g \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/submission /usr/local/bin/submission
CMD ["submission"]