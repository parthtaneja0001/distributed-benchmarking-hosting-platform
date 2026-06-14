FROM golang:1.22-alpine AS builder
RUN apk add --no-cache gcc musl-dev
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN go build -o /orchestrator ./cmd/main.go

FROM alpine:latest
RUN apk add --no-cache ca-certificates docker-cli
COPY --from=builder /orchestrator /usr/local/bin/orchestrator
CMD ["orchestrator"]